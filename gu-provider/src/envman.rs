//! Execution environment manager.
//!

use actix::prelude::*;
use futures::{future, prelude::*};
use gu_actix::prelude::*;
use gu_model::envman::*;
use gu_net::rpc::peer::PeerSessionInfo;
use gu_net::rpc::{PublicMessage, RemotingContext, RemotingSystemService};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value as JsonValue;
use std::borrow::Cow;
use std::collections::BTreeMap;

/// Actor
#[derive(Default)]
struct EnvMan {
    create_map: BTreeMap<String, Box<CreateSender>>,
    session_update_map: BTreeMap<String, Recipient<SessionUpdate>>,
    get_sessions_map: BTreeMap<String, Recipient<GetSessions>>,
    destroy_session_map: BTreeMap<String, Recipient<DestroySession>>,
}

impl Actor for EnvMan {
    type Context = RemotingContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.bind::<CreateSession<JsonValue>>(CreateSession::<JsonValue>::ID);
        ctx.bind::<SessionUpdate>(SessionUpdate::ID);
        ctx.bind::<GetSessions>(GetSessions::ID);
        ctx.bind::<DestroySession>(DestroySession::ID);
    }
}

impl RemotingSystemService for EnvMan {}

pub trait EnvManService {
    type CreateOptions: Serialize + DeserializeOwned + Default + Send;
}

trait CreateSender {
    fn send(&self, msg: CreateSession<JsonValue>) -> Box<Future<Item = String, Error = Error>>;
}

struct CreateRecipient<T: EnvManService>(Recipient<CreateSession<T::CreateOptions>>);

impl<T: EnvManService + 'static> CreateSender for CreateRecipient<T> {
    fn send(&self, msg: CreateSession<JsonValue>) -> Box<Future<Item = String, Error = Error>> {
        match serde_json::from_value(msg.options) {
            Ok(options) => Box::new(
                self.0
                    .send(CreateSession {
                        env_type: msg.env_type,
                        image: msg.image,
                        name: msg.name,
                        tags: msg.tags,
                        note: msg.note,
                        options,
                    })
                    .flatten_fut(),
            ),
            Err(e) => Box::new(future::err(Error::IncorrectOptions(e.to_string()))),
        }
    }
}

struct Register<T>
where
    T: Actor,
{
    env_type: Cow<'static, str>,
    address: Addr<T>,
}

impl<T> Message for Register<T>
where
    T: Actor,
{
    type Result = ();
}

impl<T, Options: Serialize + DeserializeOwned + Default + Send + 'static> Handler<Register<T>>
    for EnvMan
where
    T: Actor + EnvManService<CreateOptions = Options>,
    T: Handler<CreateSession<Options>>
        + Handler<SessionUpdate>
        + Handler<GetSessions>
        + Handler<DestroySession>,
    T::Context: actix::dev::ToEnvelope<T, CreateSession<T::CreateOptions>>,
    T::Context: actix::dev::ToEnvelope<T, SessionUpdate>,
    T::Context: actix::dev::ToEnvelope<T, GetSessions>,
    T::Context: actix::dev::ToEnvelope<T, DestroySession>,
{
    type Result = ();

    fn handle(
        &mut self,
        msg: Register<T>,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<Register<T>>>::Result {
        let env_type: String = msg.env_type.into();
        self.create_map.insert(
            env_type.clone(),
            Box::new(CreateRecipient::<T>(msg.address.clone().recipient())),
        );
        self.session_update_map
            .insert(env_type.clone(), msg.address.clone().recipient());
        self.get_sessions_map
            .insert(env_type.clone(), msg.address.clone().recipient());
        self.destroy_session_map
            .insert(env_type, msg.address.recipient());
    }
}

fn extract_prefix(s: &str) -> Result<(&str, &str), Error> {
    if let Some(break_pos) = s.find("::") {
        return Ok((&s[..break_pos], &s[break_pos + 2..]));
    }
    return Err(Error::NoSuchSession(s.to_owned()));
}

impl Handler<CreateSession<JsonValue>> for EnvMan {
    type Result = ActorResponse<EnvMan, String, Error>;

    fn handle(&mut self, msg: CreateSession<JsonValue>, _ctx: &mut Self::Context) -> Self::Result {
        let env_type = msg.env_type.clone();
        if let Some(address) = self.create_map.get(&env_type) {
            return ActorResponse::r#async(
                address
                    .send(msg)
                    .and_then(move |session_id| Ok(format!("{}::{}", env_type, session_id)))
                    .into_actor(self),
            );
        }
        ActorResponse::reply(Err(Error::UnknownEnv(env_type)))
    }
}

impl Handler<SessionUpdate> for EnvMan {
    type Result = ActorResponse<EnvMan, Vec<String>, Vec<String>>;

    fn handle(&mut self, msg: SessionUpdate, _ctx: &mut Self::Context) -> Self::Result {
        let (prefix, session_id) = match extract_prefix(&msg.session_id) {
            Ok(v) => v,
            Err(_e) => {
                return ActorResponse::reply(Err(vec!["Invalid environment prefix".to_string()]));
            }
        };

        match self.session_update_map.get(prefix) {
            Some(r) => ActorResponse::r#async(
                r.send(SessionUpdate {
                    session_id: session_id.into(),
                    commands: msg.commands,
                })
                .map_err(|_e| Vec::new())
                .flatten_fut()
                .into_actor(self),
            ),
            None => ActorResponse::reply(Err(Vec::new())),
        }
    }
}

impl Handler<GetSessions> for EnvMan {
    type Result = ActorResponse<EnvMan, Vec<PeerSessionInfo>, ()>;

    fn handle(&mut self, _msg: GetSessions, _ctx: &mut Self::Context) -> Self::Result {
        fn add_sessions_prefix(
            prefix: String,
            sessions: Vec<PeerSessionInfo>,
        ) -> Vec<PeerSessionInfo> {
            sessions
                .into_iter()
                .map(|session| PeerSessionInfo {
                    id: format!("{}::{}", prefix, session.id),
                    ..session
                })
                .collect()
        }

        let j = future::join_all(
            self.get_sessions_map
                .iter()
                .map(|(k, v)| {
                    let prefix = k.to_owned();

                    v.send(GetSessions {})
                        .map_err(|_| ())
                        .flatten_fut()
                        .and_then(|sessions| Ok(add_sessions_prefix(prefix, sessions)))
                })
                .collect::<Vec<_>>(),
        );

        ActorResponse::r#async(
            j.and_then(|v: Vec<Vec<PeerSessionInfo>>| Ok(v.into_iter().flatten().collect()))
                .into_actor(self),
        )
    }
}

impl Handler<DestroySession> for EnvMan {
    type Result = ActorResponse<EnvMan, String, Error>;

    fn handle(
        &mut self,
        msg: DestroySession,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<DestroySession>>::Result {
        let (prefix, session_id) = match extract_prefix(&msg.session_id) {
            Ok(v) => v,
            Err(e) => return ActorResponse::reply(Err(e)),
        };

        match self.destroy_session_map.get(prefix) {
            Some(address) => ActorResponse::r#async(
                address
                    .send(DestroySession {
                        session_id: session_id.into(),
                        ..msg
                    })
                    .flatten_fut()
                    .into_actor(self),
            ),
            None => ActorResponse::reply(Err(Error::UnknownEnv(prefix.into()))),
        }
    }
}

pub fn register<A, IntoCowStr, Options>(env_type: IntoCowStr, address: Addr<A>)
where
    IntoCowStr: Into<Cow<'static, str>>,
    Options: Serialize + DeserializeOwned + Default + Send + 'static,
    A: Actor + EnvManService<CreateOptions = Options>,
    A: Handler<CreateSession<Options>>
        + Handler<SessionUpdate>
        + Handler<GetSessions>
        + Handler<DestroySession>,
    A::Context: actix::dev::ToEnvelope<A, CreateSession<A::CreateOptions>>,
    A::Context: actix::dev::ToEnvelope<A, SessionUpdate>,
    A::Context: actix::dev::ToEnvelope<A, GetSessions>,
    A::Context: actix::dev::ToEnvelope<A, DestroySession>,
{
    EnvMan::from_registry().do_send(Register {
        env_type: env_type.into(),
        address,
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_split() {
        let (p, s) = extract_prefix("hd::12345").unwrap();
        assert_eq!(p, "hd");
        assert_eq!(s, "12345");
    }
}
