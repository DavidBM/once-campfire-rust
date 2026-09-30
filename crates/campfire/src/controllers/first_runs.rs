//! `FirstRunsController` (reference/app/controllers/first_runs_controller.rb): set up the account
//! and its first administrator.

use campfire_db::{Account, FirstRun, PasswordDigest};
use campfire_kit::{Ctx, Error, Result, StatusCode, format, halt};
use campfire_views::first_runs;

use super::presenters::attachments::{self, Assignment, Record};
use crate::app::AppCtx;
use crate::concerns::{self, Before};
use crate::controllers::presenters::page::framed_page;

/// `allow_unauthenticated_access`, `before_action :prevent_repeats`
pub async fn show(c: &mut Ctx) -> Result {
    concerns::before_actions(c, Before::default().allow_unauthenticated_access()).await?;
    prevent_repeats(c).await?;
    c.respond_to(&[&format::HTML])?;
    framed_page!(c, StatusCode::OK, |ctx| first_runs::Show { ctx }).await
}

pub async fn create(c: &mut Ctx) -> Result {
    concerns::before_actions(c, Before::default().allow_unauthenticated_access()).await?;
    prevent_repeats(c).await?;

    // params.require(:user).permit(:name, :avatar, :email_address, :password)
    let user = c.params.require("user")?.permit(&campfire_kit::permit_keys(&["name", "avatar", "email_address", "password"]));
    let name = user.get("name").and_then(|p| p.to_s());
    let email_address = user.get("email_address").and_then(|p| p.to_s()).unwrap_or_default();
    let password = user.get("password").and_then(|p| p.to_s()).unwrap_or_default();
    let avatar = Assignment::from_params(&user, "avatar")?;
    // users.name is NOT NULL: Rails raises ActiveRecord::NotNullViolation.
    let Some(name) = name else { return Err(Error::internal(anyhow::anyhow!("NOT NULL constraint failed: users.name"))) };
    let avatar = avatar.stage(c.app()).await?;
    let password_digest = PasswordDigest::hash(password, c.app().db.env().bcrypt_cost).await.map_err(Error::internal)?;

    let result = c
        .app()
        .db
        .write(move |tx| {
            let administrator = FirstRun::create(tx, &name, &email_address, password_digest)?;
            let pending = attachments::assign(tx, Record::user(administrator.id), "avatar", avatar)?;
            Ok((administrator, pending))
        })
        .await;

    let root = c.url_for(&campfire_routes::root());
    match result {
        Ok((administrator, pending)) => {
            attachments::analyze_later(c.app(), pending);
            concerns::start_new_session_for(c, administrator).await?;
            c.redirect_to(&root)
        }
        // rescue ActiveRecord::RecordNotUnique
        Err(error) if error.is_record_not_unique() => c.redirect_to(&root),
        Err(error) => Err(Error::internal(error)),
    }
}

/// `redirect_to root_url if Account.any?`
async fn prevent_repeats(c: &mut Ctx) -> Result<()> {
    let any = c.app().read(|conn| Ok(Account::count(conn)? > 0)).await?;
    if any {
        let root = c.url_for(&campfire_routes::root());
        return halt(c.redirect_to(&root)?);
    }
    Ok(())
}
