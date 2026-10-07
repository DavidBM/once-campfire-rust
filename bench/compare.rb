# Native production comparison. Outputs stay in ignored tmp; never benchmarks a stub app.
require_relative "comparison_support"
require_relative "http_client"
require_relative "validate_acks"
require_relative "contracts"
require "digest"
require "time"
include BenchmarkSupport

repo = File.expand_path("..", __dir__)
workspace = File.dirname(repo)
work = File.join(repo, "tmp/bench")
options = { apps: "rails,django,laravel,express,elixir,go,rust,c", rounds: 3, duration: 8, concurrencies: "16", port: 25130,
  seed: File.join(repo, "parity/.seed/default"), preflight: false, keep_runtime: false,
  loadgen: ENV.fetch("LOADGEN", File.join(repo, "target/bench/release/loadgen")),
  env_file: ENV.fetch("BENCH_ENV_FILE", File.join(repo, "parity/.env.reference")),
  output: File.join(work, "results"), cpus: "8-11", client_cpus: "12-15",
  routes: "room_show,messages_page,sidebar,search,avatar,static_css,up,post_message" }
OptionParser.new do |parser|
  options.each do |key, default|
    if [true, false].include?(default)
      parser.on("--#{key}") { options[key] = true }
    else
      type = default.is_a?(Integer) ? Integer : String
      parser.on("--#{key.to_s.tr('_', '-')} VALUE", type) { |value| options[key] = value }
    end
  end
  parser.on("--help") { puts parser; exit }
end.parse!
raise "use at least two rounds" unless options[:rounds] >= 2
apps = options[:apps].split(",")
allowed = %w[rails django laravel express express-bun elixir go rust c]
raise "apps must be nonempty, unique and supported" unless !apps.empty? && apps.uniq == apps && apps.all? { |app| allowed.include?(app) }
selected_routes = options[:routes].split(",")
raise "unknown or empty route selection" unless !selected_routes.empty? && (selected_routes - %w[room_show messages_page sidebar search avatar static_css up post_message]).empty?
raise "duration and concurrencies must be positive" unless options[:duration].positive? && !options[:concurrencies].split(",").empty? && options[:concurrencies].split(",").all? { |value| Integer(value).positive? }
runtimes = apps.to_h { |app| [app, app == "express-bun" ? "express" : app] }
env_name = ->(app) { app.upcase.tr("-", "_") }
labels = JSON.parse(File.read(File.join(options[:seed], "labels.json")))
original_seed_sha = Digest::SHA256.file(File.join(options[:seed], "db/production.sqlite3")).hexdigest
room = Integer(labels.fetch("rooms.watercooler"))
write_room = Integer(labels.fetch("rooms.hq"))
base = "http://127.0.0.1:#{options[:port]}"
fixture_env = File.readlines(options[:env_file], chomp: true).reject { |line| line.empty? || line.start_with?("#") }.to_h { |line| line.split("=", 2) }
lg = ->(*args) do
  if ENV["LOADGEN_DEBUG"]
    output, errors, status = Open3.capture3("taskset", "-c", options[:client_cpus], options[:loadgen], *args)
    File.open(File.join(work, "loadgen-debug-#{Process.pid}.log"), "ab") { |file| file.write(errors) }
    raise "load generator failed" unless status.success?
    JSON.parse(output)
  else
    JSON.parse(run("taskset", "-c", options[:client_cpus], options[:loadgen], *args))
  end
end
container = "cf-native-bench-#{Process.pid}"
results = []
metadata = { response_validation: "route-contract-v1", started_at: Time.now.utc.iso8601, seed_sha256: original_seed_sha, server_cpus: options[:cpus],
  client_cpus: options[:client_cpus], network: "host", gzip: true, duration: options[:duration],
  concurrencies: options[:concurrencies], rounds: options[:rounds], loadgen_sha256: Digest::SHA256.file(options[:loadgen]).hexdigest,
  routes: options[:routes], images: {}, image_labels: {}, source_revisions: {}, preflight_only: options[:preflight] }
sql = ->(db, query) do
  readonly = query.match?(/\ASELECT/i)
  output = run("sqlite3", "-cmd", ".timeout 10000", *(readonly ? ["-readonly"] : []), "-json", db, query)
  output.strip.empty? ? [] : JSON.parse(output)
end
check_sample = ->(name, value) do
  raise "#{name}: unsuccessful requests #{value}" unless value.fetch("errors").zero? && value.fetch("invalid_responses").zero? && value.fetch("validation") == "route-contract-v1" && value.fetch("ok") > 0 && value.fetch("statuses") == { "200" => value.fetch("ok") }
end
begin
  options[:rounds].times do |iteration|
    order = iteration.even? ? apps : apps.reverse
    order.each do |app|
      kind = runtimes.fetch(app)
      images = { "rails" => "once-campfire:app", "rust" => "campfire-rust:app", "elixir" => "campfire-elixir:app", "express-bun" => "once-campfire-express:bun" }
      image = ENV.fetch("#{env_name.(app)}_IMAGE", images.fetch(app, "once-campfire-#{app}:app"))
      source = File.join(workspace, kind == "rails" ? "once-campfire" : "once-campfire-#{kind}")
      metadata[:images][app] = run("docker", "image", "inspect", "-f", "{{.Id}}", image).strip
      metadata[:image_labels][app] = JSON.parse(run("docker", "image", "inspect", "-f", "{{json .Config.Labels}}", image))
      metadata[:source_revisions][app] = { head: run("git", "-C", source, "rev-parse", "HEAD").strip,
        dirty: !run("git", "-C", source, "status", "--porcelain", "--untracked-files=no").strip.empty? }
      data = File.join(work, "runtime", Process.pid.to_s, "#{app}-#{iteration + 1}")
      prepare_storage(options[:seed], data)
      FileUtils.mkdir_p(File.join(data, "logs"))
      db = File.join(data, "db/production.sqlite3")
      sql.call(db, "PRAGMA user_version=1;") if kind == "c"
      sql.call(db, "UPDATE push_subscriptions SET endpoint = 'https://127.0.0.1:9/push/' || id; UPDATE webhooks SET url = 'http://127.0.0.1:9/hook/' || id;")
      raise "invalid seed" unless sql.call(db, "SELECT COUNT(*) AS n FROM messages WHERE room_id=#{room}").first.fetch("n") > 50
      initial_max_id = sql.call(db, "SELECT MAX(id) AS id FROM messages").first.fetch("id")
      initial_messages = sql.call(db, "SELECT COUNT(*) AS n FROM messages WHERE room_id=#{write_room}").first.fetch("n")
      config = fixture_env.merge("WEB_CONCURRENCY" => "3", "JOB_CONCURRENCY" => "3", "RAILS_MAX_THREADS" => "5",
        "RAILS_LOG_LEVEL" => "warn", "HTTP_PORT" => options[:port].to_s, "TARGET_PORT" => (options[:port] + 1).to_s)
      config.merge!(JSON.parse(ENV.fetch("#{env_name.(app)}_BENCH_ENV", "{}")))
      # Host-network comparisons may coexist with a release verification node.
      config["RELEASE_NODE"] = container if kind == "elixir"
      metadata[:topology] ||= {}
      metadata[:runtimes] ||= {}
      metadata[:runtimes][app] = runtimes.fetch(app)
      metadata[:topology][app] = case kind
      when "rails" then {http_workers: config.fetch("WEB_CONCURRENCY"), threads: config.fetch("RAILS_MAX_THREADS"), jobs: "Redis/Resque", cable: "Action Cable"}
      when "rust" then {processes: 1, readers: config.fetch("RAILS_MAX_THREADS"), job_workers: config.fetch("JOB_CONCURRENCY"), cable: "native tokio", jobs: "in-process"}
      when "go" then {processes: 1, cable: "native websocket", jobs: "in-process"}
      when "elixir" then {processes: 1, runtime: "BEAM", jobs: "Redis/Resque", cable: "native"}
      when "express" then {http_workers: config.fetch("WEB_WORKERS", "auto (cpuset)"), cable: "native ws with cluster IPC", jobs: "leased auxiliary SQLite"}
      when "laravel" then {http: "FrankenPHP/Octane", jobs: "leased auxiliary SQLite", cable: "native ReactPHP"}
      when "c" then {processes: 1, http_loops: config.fetch("CF_LOOPS", "affinity, capped at 4"), cache_bytes: config.fetch("CF_CACHE_BYTES", "67108864"), jobs: "in-process"}
      when "django" then {http_workers: config.fetch("WEB_WORKERS", "1"), runtime: "ASGI/Uvicorn", cable: "native", jobs: "leased auxiliary SQLite"}
      end
      command = ["docker", "run", "-d", "--name", container, "--network", "host", "--cpuset-cpus", options[:cpus]]
      command.concat environment(config)
      command.concat mounts(File.join(data, "db") => "/rails/storage/db", File.join(data, "files") => "/rails/storage/files", File.join(data, "logs") => "/rails/storage/logs")
      command << image
      run(*command)
      client = BenchmarkHTTPClient.new(base)
      deadline = clock + 90
      until client.ready?
        if clock > deadline
          FileUtils.mkdir_p(options[:output])
          logs = File.join(options[:output], "#{app}-#{iteration + 1}-startup.log")
          output, errors, = Open3.capture3("docker", "logs", container)
          File.write(logs, output + errors)
          raise "#{app} failed to start; inspect #{logs}"
        end
        sleep 0.1
      end
      run("docker", "exec", "--user", "root", container, "chmod", "-R", "a+rwX", "/rails/storage/db")
      sleep 3 unless options[:preflight]
      cookie = lg.call("login", "--base", base, "--email", labels.fetch("emails.david"), "--password", labels.fetch("passwords.all")).fetch("cookie")
      scrape = lg.call("scrape", "--base", base, "--cookie", cookie, "--room", room.to_s)
      # The load generator sends the rendered CSRF token and same-origin headers.
      csrf = scrape.fetch("csrf").to_s
      routes = { "room_show" => "/rooms/#{room}", "messages_page" => "/rooms/#{room}/messages?before=#{labels.fetch('messages.busy_060')}",
        "sidebar" => "/users/me/sidebar", "search" => "/searches?q=coffee", "avatar" => "/users/#{labels.fetch('avatar_tokens.jason')}/avatar",
        "static_css" => scrape.fetch("css"), "up" => "/up", "post_message" => nil }
      prepared = BenchmarkContracts.prepare(base, cookie, db, labels, scrape.fetch("css"), File.join(data, "contracts"))
      contracts = prepared.fetch(:contracts)
      preflight = prepared.fetch(:preflight)
      row = { app: app, round: iteration + 1, preflight: preflight, http: [], load_start: File.read("/proc/loadavg").strip }
      acknowledged_writes = 0
      write_audits = []
      unless options[:preflight]
        routes.each do |name, path|
          next unless options[:routes].split(",").include?(name)
          args = (path ? ["--path", path] : ["--post-room", write_room.to_s, "--csrf", csrf]) + ["--validate", contracts.fetch(name)]
          warm_audit = []
          unless path
            audit = File.join(data, "post-warmup-#{name}.jsonl")
            write_audits << audit
            warm_audit = ["--audit-writes", audit]
          end
          warmup = lg.call("http", "--base", base, "--cookie", cookie, *args, *warm_audit, "--conc", "4", "--duration", "2")
          check_sample.call(name, warmup)
          acknowledged_writes += warmup.fetch("ok") unless path
          options[:concurrencies].split(",").each do |concurrency|
            timed_audit = []
            unless path
              audit = File.join(data, "post-#{concurrency}.jsonl")
              write_audits << audit
              timed_audit = ["--audit-writes", audit]
            end
            value = lg.call("http", "--base", base, "--cookie", cookie, *args, *timed_audit, "--conc", concurrency, "--duration", options[:duration].to_s)
            check_sample.call(name, value)
            acknowledged_writes += value.fetch("ok") unless path
            row[:http] << value.merge("route" => name)
            puts "#{app} round #{iteration + 1}: #{name} #{concurrency} clients #{value.fetch('rps')} req/s"
            STDOUT.flush
          end
        end
      end
      actual_messages = sql.call(db, "SELECT COUNT(*) AS n FROM messages WHERE room_id=#{write_room}").first.fetch("n")
      raise "acknowledged HTTP write count mismatch" unless actual_messages - initial_messages == acknowledged_writes
      raise "FTS entry missing" unless sql.call(db, "SELECT COUNT(*) AS n FROM messages WHERE id NOT IN (SELECT rowid FROM message_search_index)").first.fetch("n").zero?
      raise "rich text entry missing" unless sql.call(db, "SELECT COUNT(*) AS n FROM messages WHERE room_id=#{write_room} AND id>#{initial_max_id} AND id NOT IN (SELECT record_id FROM active_storage_attachments WHERE record_type='Message') AND id NOT IN (SELECT record_id FROM action_text_rich_texts WHERE record_type='Message' AND name='body')").first.fetch("n").zero?
      raise "fixture corrupt" unless sql.call(db, "PRAGMA integrity_check;").first.values == ["ok"]
      richtext_posts = sql.call(db, "SELECT COUNT(*) AS n FROM messages m JOIN action_text_rich_texts rt ON rt.record_type='Message' AND rt.record_id=m.id AND rt.name='body' WHERE m.room_id=#{write_room} AND m.id>#{initial_max_id} AND rt.body LIKE '%bench write %'").first.fetch("n")
      raise "acknowledged message body missing" if richtext_posts != acknowledged_writes
      row[:post_write_audit] = AcknowledgedWrites.verify(db, write_room, write_audits, acknowledged_writes)
      row[:richtext_http_posts] = richtext_posts
      row[:persisted_writes] = actual_messages - initial_messages
      row[:load_end] = File.read("/proc/loadavg").strip
      results << row
      write_json(File.join(options[:output], "#{app}-#{iteration + 1}.json"), row)
      remove_container(container)
      FileUtils.rm_rf(data) unless options[:keep_runtime]
    end
  end
  raise "original seed changed" unless Digest::SHA256.file(File.join(options[:seed], "db/production.sqlite3")).hexdigest == original_seed_sha
  summary = apps.to_h do |app|
    rows = results.select { |row| row[:app] == app }
    values = %w[room_show messages_page sidebar search post_message].to_h do |name|
      samples = rows.filter_map { |row| row[:http].find { |item| item.fetch("route") == name && item.fetch("conc") == 16 }&.fetch("rps") }
      [name, samples.empty? ? nil : { median_rps: median(samples), runs: samples }]
    end
    [app, values]
  end
  write_json(File.join(options[:output], "summary.json"), metadata: metadata, results: summary)
  puts JSON.pretty_generate(summary)
ensure
  remove_container(container)
end
