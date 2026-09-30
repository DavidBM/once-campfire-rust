//! `Users::PushSubscriptions::TestNotificationsController`
//! (reference/app/controllers/users/push_subscriptions/test_notifications_controller.rb).

use campfire_db::{Membership, PushSubscription};
use campfire_kit::{Ctx, Error, Result};
use ruby_compat::integer_cast;

use crate::app::AppCtx;
use crate::concerns::{self, Before};
use crate::integrations::net::Network;
use crate::integrations::web_push;

/// `@push_subscription.notification(title: "Campfire Test", body: Random.uuid, path: user_push_subscriptions_url).deliver`
pub async fn create(c: &mut Ctx) -> Result {
    concerns::before_actions(c, Before::default()).await?;
    let user_id = concerns::require_current_user(c)?.id;
    // `Current.user.push_subscriptions.find(params[:push_subscription_id])`
    let id = c.param_str("push_subscription_id").and_then(integer_cast).ok_or(Error::NotFound)?;
    let (subscription, badge) = c
        .app()
        .db
        .read(move |conn| match PushSubscription::find(conn, id) {
            Ok(subscription) if subscription.user_id == user_id => Ok(Some((subscription, Membership::unread_count(conn, user_id)?))),
            Ok(_) | Err(campfire_db::Error::RecordNotFound(_)) => Ok(None),
            Err(error) => Err(error),
        })
        .await
        .map_err(Error::internal)?
        .ok_or(Error::NotFound)?;

    let location = c.url_for(&campfire_routes::user_push_subscriptions());
    let web_push = c.app().web_push.as_ref().ok_or_else(|| Error::internal(anyhow::anyhow!("Web Push is off (no valid VAPID keys)")))?;
    web_push::deliver_test_notification(&Network::system(), web_push.vapid(), &subscription, badge, &location)
        .await
        .map_err(|error| Error::internal(anyhow::anyhow!("{error:?}")))?;
    c.redirect_to(&location)
}
