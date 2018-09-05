use std::borrow::Cow;
use actix::Message;
use errors::Result;
use std::net::SocketAddr;
use std::collections::HashSet;

/// Struct describing single service in .local domain's network
///
/// Service Instance Name = <Instance> . <Service> . <Domain>
#[derive(Debug)]
pub struct Service {
    /// Instance name; eg. "gu-provider"
    instance: Cow<'static, str>,
    /// Service type; eg. "_http._tcp"
    service: Cow<'static, str>,
}

impl Service {
    pub fn new<A, B>(instance: A, service: B) -> Self
        where
            A: Into<Cow<'static, str>>,
            B: Into<Cow<'static, str>>,
    {
        Service {
            instance: instance.into(),
            service: service.into(),
        }
    }

    pub(crate) fn to_string(&self) -> String {
        format!("{}.{}.local", self.instance, self.service)
    }
}

impl Message for Service {
    type Result = Result<HashSet<ServiceInstance>>;
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ServiceInstance {
    pub addr: SocketAddr,
}

#[derive(Debug)]
pub(crate) struct Services {
    name: String,
    set: HashSet<ServiceInstance>,
}

impl Services {
    pub(crate) fn new(name: String) -> Self {
        Services {
            name,
            set: HashSet::new(),
        }
    }

    pub(crate) fn add_instance(&mut self, name: String, instance: ServiceInstance) {
        if name == self.name {
            self.set.insert(instance);
        }
    }

    pub(crate) fn set(self) -> HashSet<ServiceInstance> {
        self.set
    }
}


#[cfg(test)]
mod tests {
    use service::Services;
    use service::ServiceInstance;
    use std::net::Ipv4Addr;
    use std::net::SocketAddrV4;

    #[test]
    fn add_matching_instance() {
        let mut services = Services::new("name".to_string());

        let ip = Ipv4Addr::new(224, 0, 0, 251);
        let address = SocketAddrV4::new(ip, 0);
        let instance = ServiceInstance {
            addr: address.into(),
        };

        services.add_instance("name".to_string(), instance);
        assert_eq!(services.set.len(), 1);
    }

    #[test]
    fn add_other_instance() {
        let mut services = Services::new("name".to_string());

        let ip = Ipv4Addr::new(224, 0, 0, 251);
        let address = SocketAddrV4::new(ip, 0);
        let instance = ServiceInstance {
            addr: address.into(),
        };

        services.add_instance("other_name".to_string(), instance);
        assert_eq!(services.set.len(), 0);
    }
}
