//! This Example has the Purpose to Show a Implementation for using the
//! `unstable-ws-subscriptions-transport` in the implementation logic of Subscriptions integration.
//! It will receive the input from the User terminal and Send some messages asynchronously
//! making usage of the Subscriptions Coordinator.
use chrono::Utc;
use futures::future::{AbortHandle, Abortable, Aborted};
use futures::task::SpawnExt;
use futures::{Stream, StreamExt};
use juniper::http::GraphQLRequest;
use juniper::{DefaultScalarValue, EmptyMutation, FieldError, RootNode, SubscriptionCoordinator};
use juniper_subscriptions::{
    ws_util::{
        GraphQLOverWebSocketMessage, GraphQLPayload, SubscriptionState, SubscriptionStateHandler,
        WsPayload,
    },
    Coordinator,
};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Handle;

#[derive(Clone)]
struct User {
    pub id: String,
}

struct Query;
#[juniper::graphql_object(Context = Context)]
impl Query {
    fn hello() -> &str {
        "hello"
    }
}

#[derive(Default, Clone)]
struct Context {
    user: Option<User>,
}

impl juniper::Context for Context {}

struct SubStateHandler;

#[derive(Deserialize, Serialize)]
struct OnConnPayload {
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
}

impl SubscriptionStateHandler<Context, std::io::Error> for SubStateHandler {
    fn handle(&self, state: SubscriptionState<Context>) -> Result<(), std::io::Error> {
        match state {
            SubscriptionState::OnConnection(payload, ctx) => {
                if let Some(payload) = payload {
                    if let Ok(payload) = serde_json::from_value::<OnConnPayload>(payload) {
                        if let Some(user_id) = payload.user_id {
                            println!(
                                "Congratulations you just identified yourself as User with ID: {}",
                                user_id
                            );
                            ctx.user = Some(User { id: user_id });
                        }
                    }
                }
            }
            SubscriptionState::OnDisconnect(_) => println!("We are Leaving!"),
            SubscriptionState::OnOperationComplete(_) => {
                println!("A Operation has been Completed!")
            }
            SubscriptionState::OnOperation(_) => println!("A Operation is Starting!"),
        }
        Ok(())
    }
}

struct Subscription;

type StringStream = Pin<Box<dyn Stream<Item = Result<String, FieldError>> + Send>>;

#[juniper::graphql_subscription(Context = Context)]
impl Subscription {
    async fn subscribe(ctx: &Context, id: i32, interval_in_seconds: i32) -> StringStream {
        println!(
            "Subscribing with id: {} and interval_in_secods: {}",
            id, interval_in_seconds
        );
        let stream = tokio::time::interval(Duration::from_secs(interval_in_seconds as u64));
        match &ctx.user {
            Some(user) => {
                let user_id = user.id.clone();
                Box::pin(
                    stream.map(move |_| {
                        Ok(format!(
                            "Hello User {} the subscribe with id: {}, is Reacting to the interval_in_seconds: {} at {}",
                            user_id,
                            id,
                            interval_in_seconds,
                            Utc::now()
                        ))
                    })
                )
            }
            None => Box::pin(stream.map(move |_| {
                Ok(format!(
                    "The subscribe with id: {}, is Reacting to the interval_in_seconds: {} at {}",
                    id,
                    interval_in_seconds,
                    Utc::now()
                ))
            })),
        }
    }
}

type Schema = RootNode<'static, Query, EmptyMutation<Context>, Subscription>;
type MyCoordinator =
    Coordinator<'static, Query, EmptyMutation<Context>, Subscription, Context, DefaultScalarValue>;

fn schema() -> Schema {
    Schema::new(Query {}, EmptyMutation::new(), Subscription {})
}

struct HandleSubscription {
    coordinator: Arc<MyCoordinator>,
    context: Context,
    has_initialized: bool,
    requests_by_id: HashMap<String, AbortHandle>,
    sub_handler: SubStateHandler,
}

impl HandleSubscription {
    fn new(coordinator: MyCoordinator) -> Self {
        Self {
            coordinator: Arc::new(coordinator),
            context: Context::default(),
            has_initialized: false,
            requests_by_id: HashMap::new(),
            sub_handler: SubStateHandler {},
        }
    }
    async fn handle(&mut self, msg: String) -> bool {
        println!("Message received: {}", msg);
        let ws_payload: WsPayload = serde_json::from_str(&msg).unwrap();
        match ws_payload.type_ {
            GraphQLOverWebSocketMessage::ConnectionInit => {
                let state = SubscriptionState::OnConnection(ws_payload.payload, &mut self.context);
                self.sub_handler.handle(state).unwrap();
                self.has_initialized = true;
            }
            GraphQLOverWebSocketMessage::Start => {
                let request_id = ws_payload.id.clone().unwrap_or("1".to_string());
                let gql_payload: GraphQLPayload<DefaultScalarValue> =
                    ws_payload.graphql_payload().unwrap();
                let mut ctx = self.context.clone();
                let state = SubscriptionState::OnOperation(&mut ctx);
                self.sub_handler.handle(state).unwrap();
                {
                    let gql_request = GraphQLRequest::<_>::new(
                        gql_payload.query.expect("Could not deserialize query"),
                        gql_payload.operation_name,
                        gql_payload.variables,
                    );
                    let req_id = request_id.clone();
                    let coord = self.coordinator.clone();
                    let future = async move {
                        println!("Initializing operation!");
                        let sub_result = coord.subscribe(&gql_request, &ctx).await;
                        let mut stream = match sub_result {
                            Ok(s) => s,
                            Err(err) => {
                                println!("{:?}", err);
                                return println!("Subscription Error");
                            }
                        };
                        while let Some(response) = stream.next().await {
                            let request_id = req_id.clone();
                            let response_text = serde_json::to_string(&response)
                                .unwrap_or("Error deserializing respone".to_owned());
                            println!("Id: {}, Response: {}", request_id, response_text);
                        }
                        ()
                    };

                    match self.requests_by_id.entry(request_id) {
                        Entry::Occupied(_o) => println!(
                            "There is a Subscription with the ID: {} already running",
                            _o.key().clone()
                        ),
                        Entry::Vacant(v) => {
                            println!("Spawning!");
                            let (abort_handle, abort_registration) = AbortHandle::new_pair();
                            tokio::spawn(Abortable::new(future, abort_registration));
                            v.insert(abort_handle);
                        }
                    }
                }
            }
            GraphQLOverWebSocketMessage::Stop => {
                let request_id = ws_payload.id.unwrap_or("1".into());
                match self.requests_by_id.entry(request_id.clone()) {
                    Entry::Occupied(o) => {
                        let abort_handle = o.remove();
                        abort_handle.abort();
                        println!("Request with ID: {} just stopped", request_id);
                        let state = SubscriptionState::OnOperationComplete(&self.context);
                        self.sub_handler.handle(state).unwrap();
                    }
                    Entry::Vacant(_) => {
                        println!("There is no Request with ID: {} ongoing", request_id)
                    }
                }
            }
            GraphQLOverWebSocketMessage::ConnectionTerminate => {
                let state = SubscriptionState::OnDisconnect(&self.context);
                self.sub_handler.handle(state).unwrap();
                return true;
            }
            _ => {
                println!("These messages should not be sent by the Client!");
            }
        };
        false
    }
}

#[tokio::main]
async fn main() {
    let coordinator: MyCoordinator = Coordinator::new(schema());
    let mut handler = HandleSubscription::new(coordinator);
    println!("Small Subscription Example With WebSocket Utilities usage!");
    let mut should_terminate = false;
    while !should_terminate {
        match get_user_action() {
            Some(action) => {
                match action {
                    SubscriptionActions::Init => {
                        let payload = match handle_init() {
                            Some(user_id) => format!("{{ \"userId\": \"{}\" }}", user_id),
                            None => format!("{{}}"),
                        };
                        should_terminate = handler
                            .handle(format!(
                                "{{ \"type\": \"connection_init\", \"payload\": {} }}",
                                payload
                            ))
                            .await;
                    }
                    SubscriptionActions::Terminate if handler.has_initialized => {
                        should_terminate = handler
                            .handle(format!(
                                "{{ \"type\": \"connection_terminate\", \"payload\": null }}"
                            ))
                            .await;
                    }
                    SubscriptionActions::Start if handler.has_initialized => {
                        match handle_start() {
                            Some(args) => {
                                let payload = format!("{{ \"query\":\"subscription {{ subscribe(id: {}, intervalInSeconds: {}) }}\" }}", args.0, args.1);
                                should_terminate = handler.handle(format!("{{ \"type\": \"start\", \"payload\": {}, \"id\": \"{}\" }}", payload, args.0)).await;
                            }
                            None => println!("You should provide valid unsigned integers"),
                        }
                    }
                    SubscriptionActions::Stop if handler.has_initialized => {
                        let req_id = handle_stop();
                        should_terminate = handler
                            .handle(format!("{{ \"type\": \"stop\", \"id\": \"{}\" }}", req_id))
                            .await;
                    }
                    _ => println!("You need to Init before anything!"),
                }
            }
            None => println!("Unknown action!"),
        }
    }
}

enum SubscriptionActions {
    Init,
    Start,
    Stop,
    Terminate,
}

fn get_user_action() -> Option<SubscriptionActions> {
    println!("Choose a action\ninit, start, stop, terminate");
    let mut action = String::new();
    std::io::stdin()
        .read_line(&mut action)
        .expect("Failed to read line");
    match action.trim().to_lowercase().as_str() {
        "init" => Some(SubscriptionActions::Init),
        "start" => Some(SubscriptionActions::Start),
        "stop" => Some(SubscriptionActions::Stop),
        "terminate" => Some(SubscriptionActions::Terminate),
        _ => None,
    }
}

fn handle_init() -> Option<String> {
    println!("Do you want to authenticate? yes/no");
    let mut should_auth = String::new();
    std::io::stdin()
        .read_line(&mut should_auth)
        .expect("Failed to read line");
    let should_auth = match should_auth.trim() {
        "yes" => true,
        _ => false,
    };
    if should_auth {
        println!("Provide your ID:");
        let mut id = String::new();
        std::io::stdin()
            .read_line(&mut id)
            .expect("Failed to read line");
        Some(id.trim().into())
    } else {
        println!("Ok we are connecting!");
        None
    }
}

fn handle_stop() -> String {
    println!("Inform the request ID to be stopped");
    let mut req_id = String::new();
    std::io::stdin()
        .read_line(&mut req_id)
        .expect("Failed to read line");
    req_id.trim().into()
}

fn handle_start() -> Option<(u32, u32)> {
    println!("Inform the ID that this Request will have:");
    let mut req_id = String::new();
    std::io::stdin()
        .read_line(&mut req_id)
        .expect("Failed to read line");
    let req_id: u32 = req_id.trim().parse().ok()?;
    println!("Inform the interval in seconds to be notified by the streaming event:");
    let mut interval = String::new();
    std::io::stdin()
        .read_line(&mut interval)
        .expect("Failed to read line");
    let interval: u32 = interval.trim().parse().ok()?;
    Some((req_id, interval))
}
