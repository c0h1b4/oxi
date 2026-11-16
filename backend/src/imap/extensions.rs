use super::connection::ImapStream;

pub(crate) async fn supports_condstore(
    session: &mut async_imap::Session<ImapStream>,
) -> async_imap::error::Result<bool> {
    let capabilities = session.capabilities().await?;
    Ok(capabilities.iter().any(|capability| {
        matches!(capability, async_imap::types::Capability::Atom(name)
            if name.eq_ignore_ascii_case("CONDSTORE"))
    }))
}

/// Keep servers without CONDSTORE on the ordinary SELECT/full-sync path.
pub(crate) async fn select_mailbox(
    session: &mut async_imap::Session<ImapStream>,
    folder: &str,
) -> async_imap::error::Result<async_imap::types::Mailbox> {
    if supports_condstore(session).await? {
        session.select_condstore(folder).await
    } else {
        session.select(folder).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imap::client::{ImapClient, RealImapClient};
    use crate::imap::connection::connect;
    use crate::imap::types::ImapCredentials;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    async fn server(
        commands: Vec<(&'static str, &'static str)>,
    ) -> (ImapCredentials, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let creds = ImapCredentials {
            host: "127.0.0.1".into(),
            port: listener.local_addr().unwrap().port(),
            tls: false,
            email: "test@example.com".into(),
            password: "secret".into(),
        };
        let task = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut socket = BufReader::new(socket);
            socket.get_mut().write_all(b"* OK ready\r\n").await.unwrap();
            let mut line = String::new();
            socket.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap();
            assert!(line.contains(" LOGIN "));
            socket
                .get_mut()
                .write_all(format!("{tag} OK logged in\r\n").as_bytes())
                .await
                .unwrap();
            for (expected, response) in commands {
                line.clear();
                socket.read_line(&mut line).await.unwrap();
                let (tag, command) = line.trim_end().split_once(' ').unwrap();
                assert_eq!(command, expected);
                socket
                    .get_mut()
                    .write_all(response.replace("{tag}", tag).as_bytes())
                    .await
                    .unwrap();
            }
        });
        (creds, task)
    }

    #[tokio::test]
    async fn select_without_condstore_supports_initial_and_repeated_selection() {
        let mut commands = Vec::new();
        for _ in 0..2 {
            commands.push((
                "CAPABILITY",
                "* CAPABILITY IMAP4rev1 IDLE\r\n{tag} OK capabilities\r\n",
            ));
            commands.push((
                "SELECT \"INBOX\"",
                "* 2 EXISTS\r\n* OK [UIDVALIDITY 1] stable\r\n{tag} OK selected\r\n",
            ));
        }
        let (creds, task) = server(commands).await;
        let mut session = connect(&creds).await.unwrap();
        for _ in 0..2 {
            let mailbox = select_mailbox(&mut session, "INBOX").await.unwrap();
            assert_eq!(mailbox.exists, 2);
            assert_eq!(mailbox.highest_modseq, None);
        }
        task.await.unwrap();
    }

    #[tokio::test]
    async fn select_preserves_condstore_when_advertised() {
        let (creds, task) = server(vec![
            (
                "CAPABILITY",
                "* CAPABILITY IMAP4rev1 condstore IDLE\r\n{tag} OK capabilities\r\n",
            ),
            (
                "SELECT \"INBOX\" (CONDSTORE)",
                "* 2 EXISTS\r\n* OK [HIGHESTMODSEQ 42] modseq\r\n{tag} OK selected\r\n",
            ),
        ])
        .await;
        let mut session = connect(&creds).await.unwrap();
        assert_eq!(
            select_mailbox(&mut session, "INBOX")
                .await
                .unwrap()
                .highest_modseq,
            Some(42)
        );
        task.await.unwrap();
    }

    #[tokio::test]
    async fn extended_status_omits_modseq_without_condstore() {
        let (creds, task) = server(vec![
            ("CAPABILITY", "* CAPABILITY IMAP4rev1 IDLE\r\n{tag} OK capabilities\r\n"),
            ("STATUS \"INBOX\" (MESSAGES UIDNEXT UIDVALIDITY UNSEEN)", "* STATUS INBOX (MESSAGES 2 UIDNEXT 3 UIDVALIDITY 1 UNSEEN 1)\r\n{tag} OK status\r\n"),
        ]).await;
        let status = RealImapClient::default()
            .folder_status_extended(&creds, "INBOX")
            .await
            .unwrap();
        assert_eq!(status.exists, 2);
        assert_eq!(status.highest_modseq, 0);
        task.await.unwrap();
    }

    #[tokio::test]
    async fn changed_flags_rejects_unsupported_extension_for_full_sync_fallback() {
        let (creds, task) = server(vec![(
            "CAPABILITY",
            "* CAPABILITY IMAP4rev1 IDLE\r\n{tag} OK capabilities\r\n",
        )])
        .await;
        let result = RealImapClient::default()
            .fetch_changed_flags(&creds, "INBOX", 42)
            .await;
        assert!(matches!(
            result,
            Err(crate::imap::error::ImapError::ProtocolError(_))
        ));
        task.await.unwrap();
    }
}
