//! gzip and ETags for pages made mostly of cached fragments (a room's messages), without
//! recompressing or rehashing the whole page on every request.
//!
//! A page is split into parts that cover it end to end: its cached fragments (each with the few
//! bytes of text before it, when that follows another fragment) and the text in between (the
//! layout). Each part is compressed once, against the part before it as a preset dictionary, and
//! kept: deflate back-references can reach anything in the last 32 KB of output, so a piece is
//! valid wherever the same predecessor comes right before it. Pages render the same until what
//! they show changes (there are no per-request CSRF tokens), so from one request to the next a page
//! is a run of stored pieces: gzip costs a CRC and some copying, and the ETag a hash of the parts'
//! digests instead of the whole body. Compressing each part on its own would lose what consecutive
//! messages share and make a room page ~4× larger; chained like this it's within 1% of compressing
//! the page whole.
//!
//! Fragments are known by identity (the `Arc` the fragment cache hands out), text by its SHA-256,
//! which is remembered by the text's bytes so that a page's layout isn't hashed on every request.
//! A stored piece that depends on a fragment holds a `Weak` to it, so while the piece exists that
//! address can't come back as a different fragment.

use std::borrow::Borrow;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::{BuildHasher, Hash};
use std::ops::Range;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, Weak};

use bytes::Bytes;
use flate2::{Compress, Compression, FlushCompress};
use foldhash::fast::RandomState;
use sha2::{Digest, Sha256};

/// Smaller fragments aren't worth a part of their own; they stay in the text around them.
const MIN_FRAGMENT: usize = 1024;
/// Smaller bodies without fragments are compressed afresh: storing them saves next to nothing.
const MIN_WHOLE_PAGE: usize = 1024;
/// At most this much text between two fragments travels with the second; more is a text part.
const MAX_GLUE: usize = 256;
/// Deflate's window.
const WINDOW: usize = 32 * 1024;
/// A bound on remembered fragments (a 32 MB fragment cache holds a few thousand messages).
const MAX_FRAGMENTS: usize = 8 * 1024;
/// Pieces kept per fragment, for the predecessors it's seen with: a message follows the same one in
/// its room and on a page of older messages, and other ones in search results.
const PIECES_PER_FRAGMENT: usize = 4;
/// A bound on the bytes of stored text pieces (a room page's layout is ~10 KB compressed).
const MAX_TEXT_PIECE_BYTES: usize = 16 << 20;
/// Larger pieces aren't stored: one would take a good part of a generation, and rotating
/// generations to fit it would push out the pieces pages keep using.
const MAX_STORED_TEXT_PIECE: usize = MAX_TEXT_PIECE_BYTES / 64;
/// A bound on the bytes of texts remembered with their SHA-256. A room page's texts (the layout
/// before and after its messages) are ~34 KB for each person and room, so a generation holds those
/// of ~240 rooms as people see them: every page in use at a small install. A text that isn't
/// remembered is simply hashed again.
const MAX_TEXT_BYTES: usize = 16 << 20;
/// Larger texts are hashed every time, for the same reason larger pieces aren't stored.
const MAX_STORED_TEXT: usize = MAX_TEXT_BYTES / 64;
/// What each stored entry costs beyond its bytes: the map's slot, its key and the allocation.
const ENTRY_OVERHEAD: usize = 128;

type Sha = [u8; 32];

/// A page's body split at its cached fragments, with each part's identity.
#[derive(Debug)]
pub struct PageParts {
    len: usize,
    parts: Vec<Part>,
}

#[derive(Debug)]
enum Part {
    Text {
        range: Range<usize>,
        sha: Sha,
    },
    /// `range` starts with the glue (the text since the previous fragment) and ends with the fragment.
    Fragment {
        fragment: Arc<String>,
        sha: Sha,
        glue: usize,
        range: Range<usize>,
    },
}

/// What comes right before a part, which its piece may refer back into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Before {
    Nothing,
    Fragment(usize),
    Text(Sha),
}

impl Part {
    fn range(&self) -> &Range<usize> {
        match self {
            Part::Text { range, .. } | Part::Fragment { range, .. } => range,
        }
    }

    /// The identity of the part as the part after it sees it, and the bytes its piece may use as
    /// a dictionary: a fragment's own bytes (never its glue), or the text.
    fn as_before<'a>(&self, body: &'a [u8]) -> (Before, &'a [u8]) {
        match self {
            Part::Text { range, sha } => (Before::Text(*sha), &body[range.clone()]),
            Part::Fragment { fragment, range, .. } => {
                (Before::Fragment(fragment_key(fragment)), &body[range.end - fragment.len()..range.end])
            }
        }
    }
}

impl PageParts {
    /// Splits `body` at `fragments` (cached HTML, in body order), or `None` when none of them is
    /// in it as is (a fragment escaped into a turbo stream, say, isn't).
    pub fn new(body: &[u8], fragments: &[Arc<String>]) -> Option<Self> {
        let located = locate(body, fragments);
        if located.is_empty() {
            return None;
        }
        let shas = fragment_shas(located.iter().map(|(fragment, _)| *fragment));
        let mut parts = Vec::with_capacity(located.len() * 2 + 1);
        let mut position = 0;
        for ((fragment, start), sha) in located.into_iter().zip(shas) {
            let follows_fragment = matches!(parts.last(), Some(Part::Fragment { .. }));
            let glue = if follows_fragment && start - position <= MAX_GLUE {
                start - position
            } else {
                if start > position {
                    parts.push(text_part(body, position..start));
                }
                0
            };
            let end = start + fragment.len();
            parts.push(Part::Fragment { fragment: fragment.clone(), sha, glue, range: start - glue..end });
            position = end;
        }
        if position < body.len() {
            parts.push(text_part(body, position..body.len()));
        }
        Some(Self { len: body.len(), parts })
    }

    /// A body without cached fragments as one text part, so its compressed form is kept and reused
    /// while the page stays the same. `None` for bodies too small to be worth storing.
    pub fn whole(body: &[u8]) -> Option<Self> {
        (body.len() >= MIN_WHOLE_PAGE).then(|| Self { len: body.len(), parts: vec![text_part(body, 0..body.len())] })
    }

    /// Whether these parts were made from a body of this length (a HEAD response has none).
    pub fn fits(&self, body: &[u8]) -> bool {
        self.len == body.len()
    }

    /// The weak ETag's value: 32 hex digits of a SHA-256 over the parts (the same body split the
    /// same way always gets the same one), as `Rack::ETag`'s is of the body.
    pub fn etag(&self, body: &[u8]) -> String {
        let mut hasher = Sha256::new();
        for part in &self.parts {
            match part {
                Part::Text { range, sha } => {
                    hasher.update(b"T");
                    hasher.update((range.len() as u64).to_le_bytes());
                    hasher.update(sha);
                }
                Part::Fragment { sha, glue, range, fragment } => {
                    hasher.update(b"F");
                    hasher.update((*glue as u64).to_le_bytes());
                    hasher.update(&body[range.start..range.start + glue]);
                    hasher.update((fragment.len() as u64).to_le_bytes());
                    hasher.update(sha);
                }
            }
        }
        hex::encode(&hasher.finalize()[..16])
    }

    /// The whole gzip member for `body`, decoding to exactly `body`. `mtime` and the Unix OS code
    /// go in the header, as `Zlib::GzipWriter` writes them.
    pub fn gzip(&self, body: &[u8], mtime: u32) -> Vec<u8> {
        let pieces = self.pieces(body);
        let mut out = Vec::with_capacity(pieces.iter().map(Bytes::len).sum::<usize>() + 20);
        out.extend_from_slice(&[0x1f, 0x8b, 8, 0]);
        out.extend_from_slice(&mtime.to_le_bytes());
        out.extend_from_slice(&[0, 3]);
        for piece in &pieces {
            out.extend_from_slice(piece);
        }
        // An empty final block (fixed Huffman), after the sync flushes that ended every piece.
        out.extend_from_slice(&[0x03, 0x00]);
        // One pass over the whole body is cheaper than combining a CRC per piece.
        out.extend_from_slice(&crc32fast::hash(body).to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out
    }

    /// Each part's compressed piece: stored ones where they fit, the rest compressed and stored.
    fn pieces(&self, body: &[u8]) -> Vec<Bytes> {
        let befores: Vec<(Before, &[u8])> =
            std::iter::once((Before::Nothing, &b""[..])).chain(self.parts.iter().map(|part| part.as_before(body))).collect();
        // Only look up under the locks; compressing happens outside them.
        let stored: Vec<Option<Bytes>> = {
            let fragments = lock(&FRAGMENTS);
            let mut texts = lock(&TEXT_PIECES);
            self.parts
                .iter()
                .zip(&befores)
                .map(|(part, (before, _))| match part {
                    Part::Text { sha, .. } => texts.get(&(*sha, *before), |piece| piece.deflated.clone()),
                    Part::Fragment { fragment, glue, range, .. } => {
                        let glue = &body[range.start..range.start + glue];
                        fragments.get(&fragment_key(fragment)).and_then(|known| known.piece_after(*before, glue))
                    }
                })
                .collect()
        };
        let mut pieces = Vec::with_capacity(self.parts.len());
        let mut new_texts = Vec::new();
        let mut new_fragments = Vec::new();
        for (index, ((part, (before, dictionary)), stored)) in self.parts.iter().zip(&befores).zip(stored).enumerate() {
            if let Some(piece) = stored {
                pieces.push(piece);
                continue;
            }
            let deflated = compress(dictionary, &body[part.range().clone()]);
            let pin = match index.checked_sub(1).map(|previous| &self.parts[previous]) {
                Some(Part::Fragment { fragment, .. }) => Some(Arc::downgrade(fragment)),
                _ => None,
            };
            match part {
                Part::Text { .. } if deflated.len() > MAX_STORED_TEXT_PIECE => {}
                Part::Text { sha, .. } => new_texts.push(((*sha, *before), TextPiece { deflated: deflated.clone(), _pin: pin })),
                Part::Fragment { fragment, glue, range, .. } => new_fragments.push((
                    fragment.clone(),
                    FragmentPiece {
                        before: *before,
                        _pin: pin,
                        glue: body[range.start..range.start + glue].into(),
                        deflated: deflated.clone(),
                    },
                )),
            }
            pieces.push(deflated);
        }
        if !new_texts.is_empty() {
            let mut texts = lock(&TEXT_PIECES);
            for (key, piece) in new_texts {
                texts.insert(key, piece);
            }
        }
        if !new_fragments.is_empty() {
            let mut fragments = lock(&FRAGMENTS);
            for (fragment, piece) in new_fragments {
                if let Some(known) = fragments.get_mut(&fragment_key(&fragment)) {
                    known.store(piece);
                }
            }
        }
        pieces
    }
}

fn text_part(body: &[u8], range: Range<usize>) -> Part {
    let sha = text_sha(&TEXT_SHAS, &body[range.clone()]);
    Part::Text { range, sha }
}

/// The SHA-256 of `text`, remembered by its bytes: a page repeats its texts (a room's layout) from
/// one request to the next, and finding one again costs a fast hash and a compare, a small part of
/// hashing it. The compare is what makes this sound, since the SHA-256 is what stored pieces are
/// found by: two texts whose fast hashes collide still get their own.
fn text_sha<S: BuildHasher + Default>(shas: &Mutex<Generations<Box<[u8]>, Sha, S>>, text: &[u8]) -> Sha {
    if let Some(sha) = lock(shas).get(text, |sha| *sha) {
        return sha;
    }
    let sha = Sha256::digest(text).into();
    if text.len() <= MAX_STORED_TEXT {
        lock(shas).insert(text.into(), sha);
    }
    sha
}

/// Where each fragment is, searching after the previous one; fragments not found are skipped.
fn locate<'a>(body: &[u8], fragments: &'a [Arc<String>]) -> Vec<(&'a Arc<String>, usize)> {
    let mut located = Vec::new();
    let mut position = 0;
    for fragment in fragments.iter().filter(|f| f.len() >= MIN_FRAGMENT) {
        if let Some(start) = find(body, position, fragment.as_bytes()) {
            located.push((fragment, start));
            position = start + fragment.len();
        }
    }
    located
}

/// The first occurrence of `needle` (never empty: `locate` passes only fragments of at least
/// [`MIN_FRAGMENT`] bytes) in `body` at or after `from`. On a page of messages the next fragment
/// nearly always starts within [`MAX_GLUE`] bytes of the previous one, and scanning that window
/// first is cheaper than setting up a substring search, which is paid per fragment. The window's
/// starts are tried in order before the search goes past it, so the first occurrence still wins.
fn find(body: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    let near = &body[from..body.len().min(from + MAX_GLUE + 1)];
    memchr::memchr_iter(needle[0], near)
        .map(|at| from + at)
        .find(|&start| body[start..].starts_with(needle))
        .or_else(|| find_far(body, from + near.len(), needle))
}

/// [`find`] past the window. Finding a short prefix and comparing the rest is much cheaper than a
/// whole-needle search, whose setup is linear in the needle.
fn find_far(body: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    let prefix = &needle[..needle.len().min(64)];
    let finder = memchr::memmem::Finder::new(prefix);
    let mut at = from;
    while let Some(found) = finder.find(&body[at..]) {
        let start = at + found;
        if body[start..].starts_with(needle) {
            return Some(start);
        }
        at = start + 1;
    }
    None
}

/// Raw deflate of `text` at level 6 with `dictionary` (its last 32 KB) preset, sync-flushed so it
/// ends on a byte boundary with no final block.
fn compress(dictionary: &[u8], text: &[u8]) -> Bytes {
    let mut deflate = Compress::new(Compression::default(), false);
    if !dictionary.is_empty() {
        deflate
            .set_dictionary(&dictionary[dictionary.len().saturating_sub(WINDOW)..])
            .expect("a fresh raw deflate stream takes a dictionary");
    }
    let mut out = Vec::with_capacity(text.len() / 4 + 64);
    loop {
        let consumed = deflate.total_in() as usize;
        if out.capacity() - out.len() < 1024 {
            out.reserve(out.capacity().max(4096));
        }
        deflate.compress_vec(&text[consumed..], &mut out, FlushCompress::Sync).expect("deflate doesn't fail on valid input");
        // Done once all input is in and the flush left spare room in the output.
        if deflate.total_in() as usize == text.len() && out.len() < out.capacity() {
            break;
        }
    }
    // Stored pieces live on, so they keep only what they use (`Bytes` keeps a `Vec`'s capacity).
    out.shrink_to_fit();
    out.into()
}

// --- What's remembered -------------------------------------------------------------------------

/// A fragment seen in a page: its SHA-256, and its pieces for the predecessors it's followed, most
/// recently stored last.
struct KnownFragment {
    fragment: Weak<String>,
    sha: Sha,
    pieces: Vec<Arc<FragmentPiece>>,
}

struct FragmentPiece {
    before: Before,
    /// Keeps a predecessor fragment's address from being reused while this piece refers to it.
    _pin: Option<Weak<String>>,
    glue: Box<[u8]>,
    deflated: Bytes,
}

impl KnownFragment {
    fn piece_after(&self, before: Before, glue: &[u8]) -> Option<Bytes> {
        self.pieces.iter().find(|piece| piece.before == before && *piece.glue == *glue).map(|piece| piece.deflated.clone())
    }

    /// Stores `piece`, dropping the oldest when there are already [`PIECES_PER_FRAGMENT`].
    fn store(&mut self, piece: FragmentPiece) {
        self.pieces.retain(|stored| stored.before != piece.before || stored.glue != piece.glue);
        if self.pieces.len() == PIECES_PER_FRAGMENT {
            self.pieces.remove(0);
        }
        self.pieces.push(Arc::new(piece));
    }
}

struct TextPiece {
    deflated: Bytes,
    _pin: Option<Weak<String>>,
}

/// A map in two generations, bounded by what its entries cost: reading an old entry promotes it,
/// and when the young generation costs more than half the budget it becomes the old one (dropping
/// the previous old one). Bounded, and what pages keep using stays. Hashed with foldhash: text keys
/// are tens of KB, and SipHash would take a third as long as the SHA-256 finding them saves.
struct Generations<K, V, S = RandomState> {
    young: HashMap<K, V, S>,
    old: HashMap<K, V, S>,
    young_cost: usize,
    budget: usize,
    cost: fn(&K, &V) -> usize,
}

impl<K: Hash + Eq, V, S: BuildHasher + Default> Generations<K, V, S> {
    fn with_budget(budget: usize, cost: fn(&K, &V) -> usize) -> Self {
        Self { young: HashMap::default(), old: HashMap::default(), young_cost: 0, budget, cost }
    }

    /// What `read` makes of the entry for `key`, promoting it if it's old.
    fn get<Q, R>(&mut self, key: &Q, read: impl FnOnce(&V) -> R) -> Option<R>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if let Some(value) = self.young.get(key) {
            return Some(read(value));
        }
        let (key, value) = self.old.remove_entry(key)?;
        let found = read(&value);
        self.insert(key, value);
        Some(found)
    }

    fn insert(&mut self, key: K, value: V) {
        self.young_cost += (self.cost)(&key, &value);
        match self.young.entry(key) {
            // Another request stored the same meanwhile.
            Entry::Occupied(mut entry) => {
                self.young_cost -= (self.cost)(entry.key(), entry.get());
                entry.insert(value);
            }
            Entry::Vacant(entry) => {
                entry.insert(value);
            }
        }
        if self.young_cost > self.budget / 2 {
            self.old = std::mem::take(&mut self.young);
            self.young_cost = 0;
        }
    }
}

static FRAGMENTS: LazyLock<Mutex<HashMap<usize, KnownFragment>>> = LazyLock::new(Mutex::default);
static TEXT_PIECES: LazyLock<Mutex<Generations<(Sha, Before), TextPiece>>> =
    LazyLock::new(|| Mutex::new(Generations::with_budget(MAX_TEXT_PIECE_BYTES, |_, piece| piece.deflated.len() + ENTRY_OVERHEAD)));
static TEXT_SHAS: LazyLock<Mutex<Generations<Box<[u8]>, Sha>>> =
    LazyLock::new(|| Mutex::new(Generations::with_budget(MAX_TEXT_BYTES, |text, _| text.len() + ENTRY_OVERHEAD)));

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn fragment_key(fragment: &Arc<String>) -> usize {
    Arc::as_ptr(fragment) as usize
}

/// Each fragment's SHA-256, hashing (and remembering) the ones not seen before. A remembered
/// entry at the same address is the same fragment while its `Weak` keeps the address taken.
fn fragment_shas<'a>(fragments: impl Iterator<Item = &'a Arc<String>> + Clone) -> Vec<Sha> {
    let mut known = lock(&FRAGMENTS);
    let missing = fragments.clone().filter(|f| !known.contains_key(&fragment_key(f))).count();
    if known.len() + missing > MAX_FRAGMENTS {
        known.retain(|_, entry| entry.fragment.strong_count() > 0);
        if known.len() + missing > MAX_FRAGMENTS {
            known.clear();
        }
    }
    fragments
        .map(|fragment| {
            known
                .entry(fragment_key(fragment))
                .or_insert_with(|| KnownFragment {
                    fragment: Arc::downgrade(fragment),
                    sha: Sha256::digest(fragment.as_bytes()).into(),
                    pieces: Vec::new(),
                })
                .sha
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn gunzip(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes).read_to_end(&mut out).unwrap();
        out
    }

    fn message(n: usize) -> Arc<String> {
        Arc::new(format!("<div id=\"message_{n}\" class=\"message\">{}</div>\n", "<button>Boost</button> hello there ".repeat(40 + n % 7)))
    }

    fn page(head: &str, messages: &[Arc<String>], tail: &str) -> String {
        let mut page = head.to_string();
        for message in messages {
            page.push_str("  ");
            page.push_str(message);
        }
        page.push_str(tail);
        page
    }

    fn gzip(body: &str, fragments: &[Arc<String>]) -> Vec<u8> {
        PageParts::new(body.as_bytes(), fragments).expect("fragments in the body").gzip(body.as_bytes(), 0)
    }

    #[test]
    fn a_page_without_fragments_is_one_stored_piece() {
        let body = format!("<html><body>{}</body></html>", "<li>sidebar room</li>".repeat(200));
        let parts = PageParts::whole(body.as_bytes()).unwrap();
        let gz = parts.gzip(body.as_bytes(), 0);
        assert_eq!(gunzip(&gz), body.as_bytes());
        let sha: Sha = Sha256::digest(body.as_bytes()).into();
        assert!(lock(&TEXT_PIECES).get(&(sha, Before::Nothing), |_| ()).is_some(), "stored under the body's SHA-256");
        assert_eq!(PageParts::whole(body.as_bytes()).unwrap().gzip(body.as_bytes(), 0), gz);
        assert!(PageParts::whole(b"<p>small</p>").is_none());
    }

    #[test]
    fn decodes_to_the_body_and_reuses_pieces() {
        let messages: Vec<_> = (0..30).map(message).collect();
        let body = page("<html><head>layout</head><body>", &messages, "</body></html>");
        let parts = PageParts::new(body.as_bytes(), &messages).unwrap();
        let gz = parts.gzip(body.as_bytes(), 1234);
        assert_eq!(gunzip(&gz), body.as_bytes());
        assert_eq!(&gz[4..8], &1234u32.to_le_bytes());
        assert_eq!(gz[9], 3);
        let piece = lock(&FRAGMENTS)[&fragment_key(&messages[5])].pieces[0].clone();
        assert_eq!(
            PageParts::new(body.as_bytes(), &messages).unwrap().gzip(body.as_bytes(), 1234),
            gz,
            "the same page is the same stored pieces"
        );
        assert!(Arc::ptr_eq(&piece, &lock(&FRAGMENTS)[&fragment_key(&messages[5])].pieces[0]));
    }

    #[test]
    fn a_changed_layout_or_neighbour_decodes_correctly() {
        let messages: Vec<_> = (100..110).map(message).collect();
        let body = page("<p>", &messages, "</p>");
        gzip(&body, &messages);
        for body in [page("<p>changed", &messages, "</p>"), page("<p>", &messages, "</p>changed")] {
            assert_eq!(gunzip(&gzip(&body, &messages)), body.as_bytes());
        }
        // Drop one message: the one after it now follows a different predecessor.
        let mut fewer = messages.clone();
        fewer.remove(4);
        let body = page("<p>", &fewer, "</p>");
        assert_eq!(gunzip(&gzip(&body, &fewer)), body.as_bytes());
        // Different glue between the same fragments.
        let body: String = std::iter::once("<p>".to_string()).chain(messages.iter().map(|m| format!("\n    {m}"))).collect();
        assert_eq!(gunzip(&gzip(&body, &messages)), body.as_bytes());
    }

    #[test]
    fn missing_small_repeated_and_far_apart_fragments() {
        let messages: Vec<_> = (200..206).map(message).collect();
        let small = Arc::new("<i>small</i>".to_string());
        let absent = message(999);
        let listed = vec![messages[0].clone(), small.clone(), absent, messages[1].clone(), messages[2].clone()];
        let body = page("<p>", &[messages[0].clone(), small, messages[1].clone(), messages[2].clone()], "</p>");
        assert_eq!(gunzip(&gzip(&body, &listed)), body.as_bytes());
        assert!(PageParts::new(b"<p>nothing cached</p>", &messages).is_none());

        let (a, b) = (message(300), message(301));
        let body = format!("{a}{b}{a}{}{b}{a}", "x".repeat(MAX_GLUE + 1));
        let listed = vec![a.clone(), b.clone(), a.clone(), b.clone(), a.clone()];
        for _ in 0..2 {
            assert_eq!(gunzip(&gzip(&body, &listed)), body.as_bytes());
        }
    }

    #[test]
    fn find_is_the_first_occurrence_at_or_after_from() {
        // Bodies dense in the bytes fragments start with, so near misses and repeats are common on
        // both sides of the window.
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = |bound: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % bound as u64) as usize
        };
        for _ in 0..5_000 {
            let body: Vec<u8> = (0..1 + next(2_000)).map(|_| b"<<\n a"[next(5)]).collect();
            let (needle, from): (Vec<u8>, _) = if next(2) == 0 {
                // A needle from the body, often at either edge of the window.
                let start = next(body.len());
                let distance = [0, 1, MAX_GLUE, MAX_GLUE + 1, MAX_GLUE + 2, next(2_000)][next(6)];
                (body[start..body.len().min(start + 1 + next(300))].to_vec(), start.saturating_sub(distance))
            } else {
                ((0..1 + next(4)).map(|_| b"<<\n a"[next(5)]).collect(), next(body.len() + 1))
            };
            let first = body[from..].windows(needle.len()).position(|window| window == needle).map(|at| from + at);
            assert_eq!(find(&body, from, &needle), first, "{needle:?} from {from}");
        }
    }

    #[test]
    fn a_fragment_keeps_a_piece_for_each_predecessor() {
        let messages: Vec<_> = (600..606).map(message).collect();
        let room = page("<p>", &messages, "</p>");
        // The same last message after a different one, as in search results.
        let search_hits = vec![messages[1].clone(), messages[5].clone()];
        let search = page("<q>", &search_hits, "</q>");
        for body in [&room, &search] {
            gzip(body, if std::ptr::eq(body, &room) { &messages } else { &search_hits });
        }
        let pieces = |fragment: &Arc<String>| lock(&FRAGMENTS)[&fragment_key(fragment)].pieces.clone();
        let before = pieces(&messages[5]);
        assert_eq!(before.len(), 2, "one after message 604, one after 601");
        gzip(&room, &messages);
        gzip(&search, &search_hits);
        let after = pieces(&messages[5]);
        assert!(before.iter().zip(&after).all(|(a, b)| Arc::ptr_eq(a, b)), "both pages reuse theirs");
    }

    #[test]
    fn etags_follow_the_content() {
        let messages: Vec<_> = (400..410).map(message).collect();
        let etag = |body: &str| PageParts::new(body.as_bytes(), &messages).unwrap().etag(body.as_bytes());
        let body = page("<p>", &messages, "</p>");
        assert_eq!(etag(&body), etag(&body.clone()));
        assert_eq!(etag(&body).len(), 32);
        assert_ne!(etag(&body), etag(&page("<p>", &messages, "</p>!")));
        assert_ne!(etag(&body), etag(&page("<q>", &messages, "</p>")));
        let glued: String =
            std::iter::once("<p>".to_string()).chain(messages.iter().map(|m| format!(" {m}"))).chain(["</p>".into()]).collect();
        assert_ne!(etag(&body), etag(&glued));
    }

    #[test]
    fn stays_close_to_whole_body_compression() {
        let messages: Vec<_> = (500..540).map(message).collect();
        let body = page(&"<head>layout</head>".repeat(200), &messages, &"<footer/>".repeat(300));
        let mut whole = flate2::write::GzEncoder::new(Vec::new(), Compression::default());
        whole.write_all(body.as_bytes()).unwrap();
        let whole = whole.finish().unwrap().len();
        let spliced = gzip(&body, &messages).len();
        // Each piece costs its flush marker and block header, not a recompressed message.
        assert!(spliced < whole + 40 * (messages.len() + 2), "{spliced} bytes spliced vs {whole} whole");
    }

    #[test]
    fn generations_stay_within_their_budget_and_keep_what_is_read() {
        let budget = 64 * 1024;
        let size = 1024;
        let mut map: Generations<u8, usize> = Generations::with_budget(budget, |_, size| *size);
        for n in 0..=255u8 {
            map.insert(n, size);
            assert!(map.get(&0, |_| ()).is_some(), "what's read stays");
            let held = (map.young.len() + map.old.len()) * size;
            // Each generation may overshoot half the budget by the entry that filled it.
            assert!(held <= budget + 2 * size, "{held} bytes held");
        }
        assert!(map.get(&255, |_| ()).is_some(), "the latest is kept");
        assert!(map.get(&1, |_| ()).is_none(), "what isn't read ages out");

        let mut map: Generations<u8, usize> = Generations::with_budget(budget, |_, size| *size);
        map.insert(1, size);
        map.insert(1, size);
        assert_eq!(map.young_cost, size, "an entry stored twice counts once");
    }

    #[test]
    fn a_large_one_off_body_is_served_but_not_remembered() {
        // Incompressible, so its piece is as large as the text: over both stores' bounds.
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let body: Vec<u8> = (0..MAX_STORED_TEXT.max(MAX_STORED_TEXT_PIECE) + 1)
            .map(|_| {
                state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                (state >> 56) as u8
            })
            .collect();
        assert_eq!(gunzip(&PageParts::whole(&body).unwrap().gzip(&body, 0)), body);
        let sha: Sha = Sha256::digest(&body).into();
        assert!(lock(&TEXT_SHAS).get(&body[..], |_| ()).is_none(), "hashed, not remembered");
        assert!(lock(&TEXT_PIECES).get(&(sha, Before::Nothing), |_| ()).is_none(), "compressed, not stored");
    }

    #[test]
    fn a_text_part_is_its_texts_sha256_the_first_time_and_after() {
        let body = page("<html><head>layout</head><body>", &(700..703).map(message).collect::<Vec<_>>(), "</body></html>");
        for range in [0..10, 5..body.len(), 0..body.len()] {
            let text: Sha = Sha256::digest(&body.as_bytes()[range.clone()]).into();
            for _ in 0..2 {
                let Part::Text { sha, .. } = text_part(body.as_bytes(), range.clone()) else { unreachable!() };
                assert_eq!(sha, text);
            }
        }
    }

    /// Hashes every key alike, as texts whose fast hashes collide.
    #[derive(Default)]
    struct Colliding;

    impl std::hash::Hasher for Colliding {
        fn finish(&self) -> u64 {
            0
        }

        fn write(&mut self, _: &[u8]) {}
    }

    #[test]
    fn a_text_is_known_by_its_bytes_not_its_fast_hash() {
        // A small budget, so texts also age out and come back through the old generation.
        let shas: Mutex<Generations<Box<[u8]>, Sha, std::hash::BuildHasherDefault<Colliding>>> =
            Mutex::new(Generations::with_budget(8 * 1024, |text, _| text.len() + ENTRY_OVERHEAD));
        // The same length, all in one bucket.
        let texts: Vec<String> = (0..30).map(|n| format!("<h1>Room {n:02}</h1>").repeat(40)).collect();
        for _ in 0..3 {
            for text in &texts {
                let sha: Sha = Sha256::digest(text).into();
                assert_eq!(text_sha(&shas, text.as_bytes()), sha);
                assert_eq!(lock(&shas).get(text.as_bytes(), |sha| *sha), Some(sha), "remembered as its own");
            }
        }
    }

    #[test]
    fn stored_pieces_keep_only_what_they_use() {
        let text = "<p>repeated</p>".repeat(100_000);
        let piece = compress(b"", text.as_bytes());
        let vec: Vec<u8> = piece.into();
        assert!(vec.capacity() < 64 * 1024, "{} bytes of capacity for {}", vec.capacity(), vec.len());
    }
}
