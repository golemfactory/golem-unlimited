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
    }
}
