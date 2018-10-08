error_chain! {
    foreign_links {
        IoError(::std::io::Error);
        CanceledFutureError(::futures::Canceled);
        DnsParserError(::dns_parser::Error);
    }

    errors {
        DnsPacketBuildError(t: Vec<u8>) {
            description("invalid dns packet")
            display("invalid dns packet: '{:?}'", t)
        }

        ActorNotInitialized {
            description("actor not initialized properly")
            display("actor not initialized properly")
        }

        UninitializedChannelReceiver {
            description("actor not initialized properly")
            display("actor not initialized properly")
        }

        FutureSendError {
            description("error while trying to send message")
            display("error while trying to send message")
        }

        MissingKey {
            description("there is no such key")
            display("there is no such key")
        }

        DoSendError {
            description("cannot send message by do_send")
            display("cannot send message by do_send")
        }

        Mailbox
    }
}

use actix::MailboxError;

impl From<MailboxError> for Error {
    fn from(_: MailboxError) -> Self {
        ErrorKind::Mailbox.into()
    }
}
