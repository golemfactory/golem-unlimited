use bytes::BytesMut;
use errors::{Error, ErrorKind, Result};
use service::Service;
use service::ServiceInstance;
use tokio_codec::{Decoder, Encoder};

use dns_parser::{Builder, Packet, QueryClass, QueryType};

#[derive(Debug)]
pub struct MdnsCodec;

impl Decoder for MdnsCodec {
    type Item = ServiceInstance;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<ServiceInstance>> {
        let packet = Packet::parse(src.as_ref())?;

        info!("Received packet: {:?}", packet);
        Ok(Some(ServiceInstance {
            data: packet.header,
        }))
    }
}

impl Encoder for MdnsCodec {
    type Item = (Service, u16);
    type Error = Error;

    fn encode(&mut self, item: (Service, u16), dst: &mut BytesMut) -> Result<()> {
        let mut builder = Builder::new_query(item.1, false);
        builder.add_question(
            item.0.to_string().as_ref(),
            true,
            QueryType::PTR,
            QueryClass::IN,
        );
        let packet = builder.build().map_err(ErrorKind::DnsPacketBuildError)?;
        info!("Encoded packet: {:?}", packet);
        let pack = Packet::parse(packet.as_ref())?;
        info!("Received packet: {:#?}", pack);

        dst.extend_from_slice(packet.as_ref());

        info!("Received packet: {:#?}", dst);
        Ok(())
    }
}
