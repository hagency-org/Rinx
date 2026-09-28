//! Fence asynchronous UI work to the Matrix session that initiated it.
use matrix_sdk::Client;

pub(crate) fn is_current(client: &Client) -> bool {
    crate::sliding_sync::get_client().is_some_and(|current| {
        // SessionMeta is stored inside the SDK's shared BaseClient. Its address
        // remains equal for Client clones, but differs after a same-user/device
        // re-login. Checking only the IDs would accept work from that old session.
        client
            .session_meta()
            .zip(current.session_meta())
            .is_some_and(|(a, b)| std::ptr::eq(a, b))
            && client.user_id().is_some()
            && current.user_id() == client.user_id()
            && current.device_id() == client.device_id()
            && current.homeserver() == client.homeserver()
    })
}

pub(crate) fn ensure_current(client: &Client) -> anyhow::Result<()> {
    anyhow::ensure!(
        is_current(client),
        "The Matrix session changed. Reopen this panel."
    );
    Ok(())
}
