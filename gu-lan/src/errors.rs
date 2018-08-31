error_chain! {
        foreign_links {
            IoError(::std::io::Error);
            CanceledFutureError(::futures::Canceled);
            DnsParserError(::dns_parser::Error);
        }

        errors {

        }
    }