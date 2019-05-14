use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt::Display,
    net::Ipv4Addr,
    result::Result as StdResult,
    str::FromStr,
};

use actix::prelude::*;
use serde::Serialize;

use errors::Result;

/// Struct describing single service in .local domain's network
///
/// Service Instance Name = <Instance> . <Service> . <Domain>
#[derive(Debug, Clone)]
pub struct ServiceDescription {
    /// Instance name; eg. "_http._tcp"
    service: Cow<'static, str>,
    /// Service type; eg. "local"
    domain: Cow<'static, str>,
}

impl ServiceDescription {
    pub fn new<A, B>(service: A, domain: B) -> Self
    where
        A: Into<Cow<'static, str>>,
        B: Into<Cow<'static, str>>,
    {
        ServiceDescription {
            service: service.into(),
            domain: domain.into(),
        }
    }

    pub(crate) fn to_string(&self) -> String {
        format!("{}.{}", self.service, self.domain)
    }
}

impl<T> From<T> for ServiceDescription
where
    T: Display,
{
    fn from(s: T) -> Self {
        ServiceDescription {
            service: format!("_gu_{}._tcp", s).into(),
            domain: "local".into(),
        }
    }
}

impl Message for ServiceDescription {
    type Result = Result<HashSet<ServiceInstance>>;
}

#[derive(Debug, Clone)]
pub struct ServicesDescription {
    services: Vec<ServiceDescription>,
}

impl ServicesDescription {
    pub fn new(services: Vec<ServiceDescription>) -> Self {
        ServicesDescription { services }
    }

    pub fn single<A, B>(instance: A, service: B) -> Self
    where
        A: Into<Cow<'static, str>>,
        B: Into<Cow<'static, str>>,
    {
        Self::new(vec![ServiceDescription::new(instance, service)])
    }

    pub(crate) fn services(&self) -> &Vec<ServiceDescription> {
        &self.services
    }
}

impl Message for ServicesDescription {
    type Result = Result<HashSet<ServiceInstance>>;
}

/// Contains information about single service in a network
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ServiceInstance {
    pub name: String,
    pub host: String,
    pub txt: Vec<String>,
    pub addrs_v4: Vec<Ipv4Addr>,
    pub ports: Vec<u16>,
}

impl ServiceInstance {
    pub fn extract<T, R>(&self, key: &T) -> Option<StdResult<R, R::Err>>
    where
        R: FromStr,
        T: AsRef<str> + ?Sized,
    {
        let key_str = format!("{}=", key.as_ref());

        self.txt
            .iter()
            .filter(|txt| txt.starts_with(&key_str))
            .map(|txt| R::from_str(&txt[key_str.len()..]))
            .next()
    }

    pub(crate) fn service(&self) -> String {
        let mut res = String::new();
        self.name.split('.').skip(1).for_each(|x| {
            res.push_str(x);
            res.push('.')
        });
        res.pop();
        res
    }
}

#[derive(Debug, Serialize, Default)]
pub struct Services {
    map: HashMap<String, HashSet<ServiceInstance>>,
}

impl<'a> From<&'a ServicesDescription> for Services {
    fn from(s: &'a ServicesDescription) -> Self {
        let mut res = Services::default();
        for service in s.services() {
            res.add_service(service.to_string());
        }

        res
    }
}

impl Services {
    pub(crate) fn add_service(&mut self, s: String) {
        self.map.insert(s, HashSet::new());
    }

    pub(crate) fn add_instance(&mut self, instance: ServiceInstance) {
        self.map
            .get_mut::<str>(instance.service().as_ref())
            .and_then(|map| Some(map.insert(instance)));
    }

    pub(crate) fn collect(self) -> HashSet<ServiceInstance> {
        let mut set: HashSet<ServiceInstance> = HashSet::new();
        for i in self.map {
            set.extend(i.1)
        }
        set
    }
}
