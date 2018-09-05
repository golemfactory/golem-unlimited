use tokio_codec::{Decoder, Encoder};
use errors::{Error, ErrorKind, Result};
use bytes::BytesMut;
use service::Service;

use dns_parser::{Builder, Packet, QueryClass, QueryType};
use dns_parser::rdata::RData::SRV;

#[derive(Debug)]
pub(crate) struct ServiceAnswer {
    pub port: u16,
    pub name: String,
}

#[derive(Debug)]
pub(crate) struct ServiceAnswers {
    pub id: u16,
    pub list: Vec<ServiceAnswer>,
}

#[derive(Debug)]
pub(crate) struct MdnsCodec;

impl Decoder for MdnsCodec {
    type Item = ServiceAnswers;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<ServiceAnswers>> {
        info!("Received packet: {:?}", src);
        let packet = Packet::parse(src.as_ref())?;
        info!("Received packet: {:?}", packet);

        let id = packet.header.id;
        let mut list = Vec::new();
        for answer in packet.answers {
            if let SRV(data) = answer.data {
                list.push(
                    ServiceAnswer {
                        port: data.port,
                        name: answer.name.to_string(),
                    });
            }
        }

        Ok(Some(ServiceAnswers {
            id,
            list,
        }))
    }
}

impl Encoder for MdnsCodec {
    type Item = (Service, u16);
    type Error = Error;

    fn encode(&mut self, item: (Service, u16), dst: &mut BytesMut) -> Result<()> {
        let mut builder = Builder::new_query(item.1, false);
        builder.add_question(item.0.to_string().as_ref(), true, QueryType::SRV, QueryClass::IN);
        let packet = builder
            .build()
            .map_err(ErrorKind::DnsPacketBuildError)?;
        info!("Encoded packet to send: {:?}", packet);

        dst.extend_from_slice(packet.as_ref());
        Ok(())
    }
}



#[cfg(test)]
mod tests {
    use mdns_codec::MdnsCodec;
    use tokio_codec::{Decoder, Encoder};
    use bytes::BytesMut;
    use service::Service;

    #[test]
    fn decode_packet() {
        let a = b"\0\0\x80\0\0\0\0\x03\0\0\0\0\x0bgu-provider\x05_http\x04_tcp\x05local\0\0!\0\x01\0\0\0<\0\x18\0\0\0\0\0P\nimapp-1023\x05local\0\nimapp-1023\x05local\0\0\x01\0\x01\0\0\0<\0\x04\n\x1e\x08\xce\nimapp-1023\x05local\0\0\x01\0\x01\0\0\0<\0\x04\xac\x11\0\x01";

        let mut bytes = BytesMut::new();
        bytes.extend_from_slice(a);

        let packet = MdnsCodec{}.decode(&mut bytes);

        assert!(packet.is_ok());
        let packet = packet.unwrap();

        assert!(packet.is_some());
        let packet = packet.unwrap();

        assert_eq!(packet.id, 0);
        assert!(packet.list.get(0).is_some());
        assert_eq!(packet.list.get(0).unwrap().name, "gu-provider._http._tcp.local");
    }

    #[test]
    fn encode_packet() {
        let service = Service::new("gu-provider", "_http._tcp");
        let mut bytes = BytesMut::new();
        let packet = MdnsCodec{}.encode((service, 124), &mut bytes);

        let a = b"\0|\0\0\0\x01\0\0\0\0\0\0\x0bgu-provider\x05_http\x04_tcp\x05local\0\0!\x80\x01";
        let mut bytes2 = BytesMut::new();
        bytes2.extend_from_slice(a);

        assert!(packet.is_ok());
        assert_eq!(bytes,  bytes2);
    }
}