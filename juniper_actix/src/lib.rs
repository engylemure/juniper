/*!

# juniper_actix

This repository contains the [actix][actix] web server integration for
[Juniper][Juniper], a [GraphQL][GraphQL] implementation for Rust, its inspired and some parts are copied from [juniper_warp][juniper_warp]

## Documentation

For documentation, including guides and examples, check out [Juniper][Juniper].

A basic usage example can also be found in the [API documentation][documentation].

## Examples

Check [examples/actix_server][example] for example code of a working actix
server with GraphQL handlers.

## Links

* [Juniper][Juniper]
* [API Reference][documentation]
* [actix][actix]

## License

This project is under the BSD-2 license.

Check the LICENSE file for details.

[actix]: https://github.com/actix/actix-web
[Juniper]: https://github.com/graphql-rust/juniper
[GraphQL]: http://graphql.org
[documentation]: https://docs.rs/juniper_actix
[example]: https://github.com/graphql-rust/juniper/blob/master/juniper_actix/examples/actix_server.rs
[juniper_warp]: https://github.com/graphql-rust/juniper/juniper_warp
*/

#![deny(missing_docs)]
#![deny(warnings)]
#![doc(html_root_url = "https://docs.rs/juniper_actix/0.1.0")]

// use futures::{FutureExt as _};
use actix_web::{
    error::{ErrorBadRequest, ErrorMethodNotAllowed, ErrorUnsupportedMediaType},
    http::{header::CONTENT_TYPE, Method},
    web, Error, FromRequest, HttpRequest, HttpResponse,
};
use juniper::{
    http::{
        graphiql::graphiql_source, playground::playground_source, GraphQLBatchRequest,
        GraphQLRequest,
    },
    ScalarValue,
};
use serde::Deserialize;

#[serde(deny_unknown_fields)]
#[derive(Deserialize, Clone, PartialEq, Debug)]
struct GetGraphQLRequest {
    query: String,
    #[serde(rename = "operationName")]
    operation_name: Option<String>,
    variables: Option<String>,
}

impl<S> From<GetGraphQLRequest> for GraphQLRequest<S>
where
    S: ScalarValue,
{
    fn from(get_req: GetGraphQLRequest) -> Self {
        let GetGraphQLRequest {
            query,
            operation_name,
            variables,
        } = get_req;
        let variables = match variables {
            Some(variables) => Some(serde_json::from_str(&variables).unwrap()),
            None => None,
        };
        Self::new(query, operation_name, variables)
    }
}

/// Actix Web GraphQL Handler for GET and POST requests
pub async fn graphql_handler<Query, Mutation, Subscription, Context, S>(
    schema: &juniper::RootNode<'static, Query, Mutation, Subscription, S>,
    context: &Context,
    req: HttpRequest,
    payload: actix_web::web::Payload,
) -> Result<HttpResponse, Error>
where
    S: ScalarValue + Send + Sync + 'static,
    Context: Send + Sync + 'static,
    Query: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
    Query::TypeInfo: Send + Sync,
    Mutation: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
    Mutation::TypeInfo: Send + Sync,
    Subscription: juniper::GraphQLSubscriptionType<S, Context = Context> + Send + Sync + 'static,
    Subscription::TypeInfo: Send + Sync,
{
    match *req.method() {
        Method::POST => post_graphql_handler(schema, context, req, payload).await,
        Method::GET => get_graphql_handler(schema, context, req).await,
        _ => Err(ErrorMethodNotAllowed(
            "GraphQL requests can only be sent with GET or POST",
        )),
    }
}
/// Actix GraphQL Handler for GET requests
pub async fn get_graphql_handler<Query, Mutation, Subscription, Context, S>(
    schema: &juniper::RootNode<'static, Query, Mutation, Subscription, S>,
    context: &Context,
    req: HttpRequest,
) -> Result<HttpResponse, Error>
where
    S: ScalarValue + Send + Sync + 'static,
    Context: Send + Sync + 'static,
    Query: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
    Query::TypeInfo: Send + Sync,
    Mutation: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
    Mutation::TypeInfo: Send + Sync,
    Subscription: juniper::GraphQLSubscriptionType<S, Context = Context> + Send + Sync + 'static,
    Subscription::TypeInfo: Send + Sync,
{
    let get_req = web::Query::<GetGraphQLRequest>::from_query(req.query_string())?;
    let req = GraphQLRequest::from(get_req.into_inner());
    let gql_response = req.execute(schema, context).await;
    let body_response = serde_json::to_string(&gql_response)?;
    let response = match gql_response.is_ok() {
        true => HttpResponse::Ok()
            .content_type("application/json")
            .body(body_response),
        false => HttpResponse::BadRequest()
            .content_type("application/json")
            .body(body_response),
    };
    Ok(response)
}

/// Actix GraphQL Handler for POST requests
pub async fn post_graphql_handler<Query, Mutation, Subscription, Context, S>(
    schema: &juniper::RootNode<'static, Query, Mutation, Subscription, S>,
    context: &Context,
    req: HttpRequest,
    payload: actix_web::web::Payload,
) -> Result<HttpResponse, Error>
where
    S: ScalarValue + Send + Sync + 'static,
    Context: Send + Sync + 'static,
    Query: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
    Query::TypeInfo: Send + Sync,
    Mutation: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
    Mutation::TypeInfo: Send + Sync,
    Subscription: juniper::GraphQLSubscriptionType<S, Context = Context> + Send + Sync + 'static,
    Subscription::TypeInfo: Send + Sync,
{
    let content_type_header = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|hv| hv.to_str().ok());
    let req = match content_type_header {
        Some("application/json") | Some("application/graphql") => {
            let body_string = String::from_request(&req, &mut payload.into_inner()).await;
            let body_string = body_string?;
            match serde_json::from_str::<GraphQLBatchRequest<S>>(&body_string) {
                Ok(req) => Ok(req),
                Err(err) => Err(ErrorBadRequest(err)),
            }
        }
        _ => Err(ErrorUnsupportedMediaType(
            "GraphQL requests should have content type `application/json` or `application/graphql`",
        )),
    }?;
    let gql_batch_response = req.execute(schema, context).await;
    let gql_response = serde_json::to_string(&gql_batch_response)?;
    let mut response = match gql_batch_response.is_ok() {
        true => HttpResponse::Ok(),
        false => HttpResponse::BadRequest(),
    };
    Ok(response.content_type("application/json").body(gql_response))
}

/// Create a handler that replies with an HTML page containing GraphiQL. This does not handle routing, so you can mount it on any endpoint
///
/// For example:
///
/// ```
/// # extern crate actix;
/// # extern crate juniper_actix;
/// #
/// # use juniper_actix::graphiql_handler;
/// # use actix_web::{web, App};
///
/// let app = App::new()
///          .route("/", web::get().to(|| graphiql_handler("/graphql", Some("/graphql/subscriptions"))));
/// ```
#[allow(dead_code)]
pub async fn graphiql_handler(
    graphql_endpoint_url: &str,
    subscriptions_endpoint_url: Option<&'static str>,
) -> Result<HttpResponse, Error> {
    let html = graphiql_source(graphql_endpoint_url, subscriptions_endpoint_url);
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

/// Create a handler that replies with an HTML page containing GraphQL Playground. This does not handle routing, so you cant mount it on any endpoint.
pub async fn playground_handler(
    graphql_endpoint_url: &str,
    subscriptions_endpoint_url: Option<&'static str>,
) -> Result<HttpResponse, Error> {
    let html = playground_source(graphql_endpoint_url, subscriptions_endpoint_url);
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

/// this is the `juniper_actix` subscriptions handler implementation
/// does not fully support the GraphQL over WS[1] specification and makes use
/// of some unsafe code for handling the sending of messages.
///
/// *Note: this implementation is in an pre-alpha state.*
///
/// [1]: https://github.com/apollographql/subscriptions-transport-ws/blob/master/PROTOCOL.md
#[cfg(feature = "subscriptions")]
pub mod subscriptions {
    use actix::{
        Actor, ActorContext, ActorFuture, AsyncContext, SpawnHandle, StreamHandler, WrapFuture,
    };
    use actix_web::{error::PayloadError, web, web::Bytes, Error, HttpRequest, HttpResponse};
    use actix_web_actors::{
        ws,
        ws::{handshake_with_protocols, WebsocketContext},
    };
    use futures::{Stream, StreamExt};
    use juniper::{http::GraphQLRequest, InputValue, ScalarValue, SubscriptionCoordinator};
    use juniper_subscriptions::ws_util::GraphQLOverWebSocketMessage;
    pub use juniper_subscriptions::ws_util::{
        EmptySubscriptionHandler, SubscriptionState, SubscriptionStateHandler,
    };
    use juniper_subscriptions::Coordinator;
    use serde::{Deserialize, Serialize};
    use std::{
        collections::HashMap,
        error::Error as StdError,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };
    use tokio::time::Duration;

    fn start<Query, Mutation, Subscription, Context, S, SubHandler, T, E>(
        actor: GraphQLWSSession<Query, Mutation, Subscription, Context, S, SubHandler, E>,
        req: &HttpRequest,
        stream: T,
    ) -> Result<HttpResponse, Error>
    where
        T: Stream<Item = Result<Bytes, PayloadError>> + 'static,
        S: ScalarValue + Send + Sync + 'static,
        Context: Clone + Send + Sync + 'static + std::marker::Unpin,
        Query: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
        Query::TypeInfo: Send + Sync,
        Mutation: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
        Mutation::TypeInfo: Send + Sync,
        Subscription:
            juniper::GraphQLSubscriptionType<S, Context = Context> + Send + Sync + 'static,
        Subscription::TypeInfo: Send + Sync,
        SubHandler: SubscriptionStateHandler<Context, E> + 'static + std::marker::Unpin,
        E: 'static + std::error::Error + std::marker::Unpin,
    {
        let mut res = handshake_with_protocols(req, &["graphql-ws"])?;
        Ok(res.streaming(WebsocketContext::create(actor, stream)))
    }

    /// Since this implementation makes usage of the unsafe keyword i will consider this as unsafe for now.
    pub async unsafe fn graphql_subscriptions<
        Query,
        Mutation,
        Subscription,
        Context,
        S,
        SubHandler,
        E,
    >(
        coordinator: web::Data<Coordinator<'static, Query, Mutation, Subscription, Context, S>>,
        context: Context,
        stream: web::Payload,
        req: HttpRequest,
        handler: Option<SubHandler>,
    ) -> Result<HttpResponse, Error>
    where
        S: ScalarValue + Send + Sync + 'static,
        Context: Clone + Send + Sync + 'static + std::marker::Unpin,
        Query: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
        Query::TypeInfo: Send + Sync,
        Mutation: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
        Mutation::TypeInfo: Send + Sync,
        Subscription:
            juniper::GraphQLSubscriptionType<S, Context = Context> + Send + Sync + 'static,
        Subscription::TypeInfo: Send + Sync,
        SubHandler: SubscriptionStateHandler<Context, E> + 'static + std::marker::Unpin,
        E: 'static + std::error::Error + std::marker::Unpin,
    {
        start(
            GraphQLWSSession {
                coordinator: coordinator.into_inner(),
                graphql_context: context,
                map_req_id_to_spawn_handle: HashMap::new(),
                has_started: Arc::new(AtomicBool::new(false)),
                handler,
                error_handler: std::marker::PhantomData,
            },
            &req,
            stream,
        )
    }

    struct GraphQLWSSession<Query, Mutation, Subscription, Context, S, SubHandler, E>
    where
        S: ScalarValue + Send + Sync + 'static,
        Context: Clone + Send + Sync + 'static + std::marker::Unpin,
        Query: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
        Query::TypeInfo: Send + Sync,
        Mutation: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
        Mutation::TypeInfo: Send + Sync,
        Subscription:
            juniper::GraphQLSubscriptionType<S, Context = Context> + Send + Sync + 'static,
        Subscription::TypeInfo: Send + Sync,
        SubHandler: SubscriptionStateHandler<Context, E> + 'static + std::marker::Unpin,
        E: 'static + std::error::Error + std::marker::Unpin,
    {
        pub map_req_id_to_spawn_handle: HashMap<String, SpawnHandle>,
        pub has_started: Arc<AtomicBool>,
        pub graphql_context: Context,
        pub coordinator: Arc<Coordinator<'static, Query, Mutation, Subscription, Context, S>>,
        pub handler: Option<SubHandler>,
        error_handler: std::marker::PhantomData<E>,
    }

    impl<Query, Mutation, Subscription, Context, S, SubHandler, E> Actor
        for GraphQLWSSession<Query, Mutation, Subscription, Context, S, SubHandler, E>
    where
        S: ScalarValue + Send + Sync + 'static,
        Context: Clone + Send + Sync + 'static + std::marker::Unpin,
        Query: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
        Query::TypeInfo: Send + Sync,
        Mutation: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
        Mutation::TypeInfo: Send + Sync,
        Subscription:
            juniper::GraphQLSubscriptionType<S, Context = Context> + Send + Sync + 'static,
        Subscription::TypeInfo: Send + Sync,
        SubHandler: SubscriptionStateHandler<Context, E> + 'static + std::marker::Unpin,
        E: 'static + std::error::Error + std::marker::Unpin,
    {
        type Context = ws::WebsocketContext<
            GraphQLWSSession<Query, Mutation, Subscription, Context, S, SubHandler, E>,
        >;
    }

    #[allow(dead_code)]
    impl<Query, Mutation, Subscription, Context, S, SubHandler, E>
        GraphQLWSSession<Query, Mutation, Subscription, Context, S, SubHandler, E>
    where
        S: ScalarValue + Send + Sync + 'static,
        Context: Clone + Send + Sync + 'static + std::marker::Unpin,
        Query: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
        Query::TypeInfo: Send + Sync,
        Mutation: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
        Mutation::TypeInfo: Send + Sync,
        Subscription:
            juniper::GraphQLSubscriptionType<S, Context = Context> + Send + Sync + 'static,
        Subscription::TypeInfo: Send + Sync,
        SubHandler: SubscriptionStateHandler<Context, E> + 'static + std::marker::Unpin,
        E: 'static + std::error::Error + std::marker::Unpin,
    {
        fn gql_connection_ack() -> String {
            let type_value =
                serde_json::to_string(&GraphQLOverWebSocketMessage::ConnectionAck).unwrap();
            format!(r#"{{"type":{}, "payload": null }}"#, type_value)
        }

        fn gql_connection_ka() -> String {
            let type_value =
                serde_json::to_string(&GraphQLOverWebSocketMessage::ConnectionKeepAlive).unwrap();
            format!(r#"{{"type":{}, "payload": null }}"#, type_value)
        }

        fn gql_connection_error() -> String {
            let type_value =
                serde_json::to_string(&GraphQLOverWebSocketMessage::ConnectionError).unwrap();
            format!(r#"{{"type":{}, "payload": null }}"#, type_value)
        }
        fn gql_error<T: StdError + Serialize>(request_id: &String, err: T) -> String {
            let type_value = serde_json::to_string(&GraphQLOverWebSocketMessage::Error).unwrap();
            format!(
                r#"{{"type":{},"id":"{}","payload":{}}}"#,
                type_value,
                request_id,
                serde_json::ser::to_string(&err)
                    .unwrap_or("Error deserializing GraphQLError".to_owned())
            )
        }

        fn gql_data(request_id: &String, response_text: String) -> String {
            let type_value = serde_json::to_string(&GraphQLOverWebSocketMessage::Data).unwrap();
            format!(
                r#"{{"type":{},"id":"{}","payload":{} }}"#,
                type_value, request_id, response_text
            )
        }

        fn gql_complete(request_id: &String) -> String {
            let type_value = serde_json::to_string(&GraphQLOverWebSocketMessage::Complete).unwrap();
            format!(
                r#"{{"type":{},"id":"{}","payload":null}}"#,
                type_value, request_id
            )
        }

        fn starting_handle(
            result: (
                GraphQLRequest<S>,
                String,
                Context,
                Arc<Coordinator<'static, Query, Mutation, Subscription, Context, S>>,
            ),
            actor: &mut Self,
            ctx: &mut ws::WebsocketContext<Self>,
        ) -> actix::fut::FutureWrap<impl futures::Future<Output = ()>, Self> {
            let ctx: *mut ws::WebsocketContext<Self> = ctx;
            let (req, req_id, gql_context, coord) = result;
            Self::handle_subscription(req, gql_context, req_id, coord, ctx).into_actor(actor)
        }

        async fn handle_subscription(
            req: GraphQLRequest<S>,
            graphql_context: Context,
            request_id: String,
            coord: Arc<Coordinator<'static, Query, Mutation, Subscription, Context, S>>,
            ctx: *mut ws::WebsocketContext<Self>,
        ) {
            // This gives access to the ctx
            // so we can send the text responses to the client,
            // in the moment this causes a Data race (UB).
            // This can be caused if two messages sent by the client
            // such as a GQL_START and a GQL_STOP of the same
            // request_id in a small period of time are sent,
            // since they are being handle asynchronously and independent of each other
            // so what can happen is that the handle of GQL_STOP will be finished
            // before the start and will not stop the starting of the stream of the GQL_START and this will not
            // stop the ongoing request, because of that, the usage of the graphql_subscriptions
            // is being considered unsafe code in the moment.
            let ctx = unsafe { ctx.as_mut().unwrap() };

            let mut values_stream = {
                let subscribe_result = coord.subscribe(&req, &graphql_context).await;
                match subscribe_result {
                    Ok(s) => s,
                    Err(err) => {
                        ctx.text(Self::gql_error(&request_id, err));
                        ctx.text(Self::gql_complete(&request_id));
                        ctx.stop();
                        return;
                    }
                }
            };
            while let Some(response) = values_stream.next().await {
                let request_id = request_id.clone();
                let response_text = serde_json::to_string(&response)
                    .unwrap_or("Error deserializing respone".to_owned());
                ctx.text(Self::gql_data(&request_id, response_text));
            }
            ctx.text(Self::gql_complete(&request_id))
        }
    }

    impl<Query, Mutation, Subscription, Context, S, SubHandler, E>
        StreamHandler<Result<ws::Message, ws::ProtocolError>>
        for GraphQLWSSession<Query, Mutation, Subscription, Context, S, SubHandler, E>
    where
        S: ScalarValue + Send + Sync + 'static,
        Context: Clone + Send + Sync + 'static + std::marker::Unpin,
        Query: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
        Query::TypeInfo: Send + Sync,
        Mutation: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
        Mutation::TypeInfo: Send + Sync,
        Subscription:
            juniper::GraphQLSubscriptionType<S, Context = Context> + Send + Sync + 'static,
        Subscription::TypeInfo: Send + Sync,
        SubHandler: SubscriptionStateHandler<Context, E> + 'static + std::marker::Unpin,
        E: 'static + std::error::Error + std::marker::Unpin,
    {
        fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
            let msg = match msg {
                Err(_) => {
                    ctx.stop();
                    return;
                }
                Ok(msg) => msg,
            };
            let has_started = self.has_started.clone();
            let has_started_value = has_started.load(Ordering::Relaxed);
            match msg {
                ws::Message::Text(text) => {
                    let m = text.trim();
                    let request: WsPayload<S> = match serde_json::from_str(m) {
                        Ok(payload) => payload,
                        Err(_) => {
                            return;
                        }
                    };
                    match request.type_ {
                        GraphQLOverWebSocketMessage::ConnectionInit => {
                            if let Some(handler) = &self.handler {
                                let state = SubscriptionState::OnConnection(
                                    Some(String::from(m)),
                                    &mut self.graphql_context,
                                );
                                let on_connect_result = handler.handle(state);
                                if let Err(_err) = on_connect_result {
                                    ctx.text(Self::gql_connection_error());
                                    ctx.stop();
                                    return;
                                }
                            }
                            ctx.text(Self::gql_connection_ack());
                            ctx.text(Self::gql_connection_ka());
                            has_started.store(true, Ordering::Relaxed);
                            ctx.run_interval(Duration::from_secs(10), |actor, ctx| {
                                let no_request = actor.map_req_id_to_spawn_handle.len() == 0;
                                if no_request {
                                    ctx.stop();
                                } else {
                                    ctx.text(Self::gql_connection_ka());
                                }
                            });
                        }
                        GraphQLOverWebSocketMessage::Start if has_started_value => {
                            let coordinator = self.coordinator.clone();
                            let mut context = self.graphql_context.clone();
                            let payload = request.payload.expect("Could not deserialize payload");
                            let request_id = request.id.unwrap_or("1".to_owned());
                            let graphql_request = GraphQLRequest::<_>::new(
                                payload.query.expect("Could not deserialize query"),
                                None,
                                payload.variables,
                            );
                            if let Some(handler) = &self.handler {
                                let state = SubscriptionState::OnOperation(&mut context);
                                handler.handle(state).unwrap();
                            }
                            {
                                use std::collections::hash_map::Entry;
                                let req_id = request_id.clone();
                                let future =
                                    async move { (graphql_request, req_id, context, coordinator) }
                                        .into_actor(self)
                                        .then(Self::starting_handle);
                                match self.map_req_id_to_spawn_handle.entry(request_id) {
                                    // Since there is another request being handle
                                    // this just ignores the start of another request with this same
                                    // request_id
                                    Entry::Occupied(_o) => (),
                                    Entry::Vacant(v) => {
                                        v.insert(ctx.spawn(future));
                                    }
                                };
                            }
                        }
                        GraphQLOverWebSocketMessage::Stop if has_started_value => {
                            let request_id = request.id.unwrap_or("1".to_owned());
                            if let Some(handler) = &self.handler {
                                let state =
                                    SubscriptionState::OnOperationComplete(&self.graphql_context);
                                handler.handle(state).unwrap();
                            }
                            match self.map_req_id_to_spawn_handle.remove(&request_id) {
                                Some(spawn_handle) => {
                                    ctx.cancel_future(spawn_handle);
                                    ctx.text(Self::gql_complete(&request_id));
                                }
                                None => {
                                    // No request with this id was found in progress.
                                    // since the Subscription Protocol Spec does not specify
                                    // what occurs in this case im just considering the possibility
                                    // of send a error.
                                    // ctx.text(Self::gql_error(
                                    //     &request_id,
                                    //     SubscriptionErrors::NoOngoingRequest(request_id.clone()),
                                    // ))
                                }
                            }
                        }
                        GraphQLOverWebSocketMessage::ConnectionTerminate => {
                            if let Some(handler) = &self.handler {
                                let state = SubscriptionState::OnDisconnect(&self.graphql_context);
                                handler.handle(state).unwrap();
                            }
                            ctx.stop();
                        }
                        _ => {}
                    }
                }
                ws::Message::Close(_) => {
                    if let Some(handler) = &self.handler {
                        let state = SubscriptionState::OnDisconnect(&self.graphql_context);
                        handler.handle(state).unwrap();
                    }
                    ctx.stop();
                }
                _ => {
                    // Non Text messages are not allowed
                    ctx.stop();
                }
            }
        }
    }

    /// Some errors specific to the Subscription logic
    /// in this implementation.
    #[allow(dead_code)]
    #[derive(Debug, Serialize)]
    enum SubscriptionErrors {
        NoOngoingRequest(String),
    }

    impl StdError for SubscriptionErrors {}

    impl std::fmt::Display for SubscriptionErrors {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            match self {
                SubscriptionErrors::NoOngoingRequest(req_id) => {
                    write!(f, "No request of id: '{}' found.", req_id)
                }
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(bound = "GraphQLPayload<S>: Deserialize<'de>")]
    struct WsPayload<S>
    where
        S: ScalarValue + Send + Sync + 'static,
    {
        id: Option<String>,
        #[serde(rename(deserialize = "type"))]
        type_: GraphQLOverWebSocketMessage,
        payload: Option<GraphQLPayload<S>>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(bound = "InputValue<S>: Deserialize<'de>")]
    struct GraphQLPayload<S>
    where
        S: ScalarValue + Send + Sync + 'static,
    {
        variables: Option<InputValue<S>>,
        extensions: Option<HashMap<String, String>>,
        #[serde(rename(deserialize = "operationName"))]
        operaton_name: Option<String>,
        query: Option<String>,
    }

    #[derive(Serialize)]
    struct Output {
        data: String,
        variables: String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{dev::ServiceResponse, http, http::header::CONTENT_TYPE, test, App};
    use futures::StreamExt;
    use juniper::{
        http::tests::{run_http_test_suite, HTTPIntegration, TestResponse},
        tests::{model::Database, schema::Query},
        EmptyMutation, EmptySubscription, RootNode,
    };

    type Schema =
        juniper::RootNode<'static, Query, EmptyMutation<Database>, EmptySubscription<Database>>;

    async fn take_response_body_string(resp: &mut ServiceResponse) -> String {
        let (response_body, ..) = resp
            .take_body()
            .map(|body_out| body_out.unwrap().to_vec())
            .into_future()
            .await;
        match response_body {
            Some(response_body) => String::from_utf8(response_body).unwrap(),
            None => String::from(""),
        }
    }

    async fn index(
        req: HttpRequest,
        payload: actix_web::web::Payload,
        schema: web::Data<Schema>,
    ) -> Result<HttpResponse, Error> {
        let context = Database::new();
        graphql_handler(&schema, &context, req, payload).await
    }

    #[actix_rt::test]
    async fn graphiql_response_does_not_panic() {
        let result = graphiql_handler("/abcd", None).await;
        assert!(result.is_ok())
    }

    #[actix_rt::test]
    async fn graphiql_endpoint_matches() {
        async fn graphql_handler() -> Result<HttpResponse, Error> {
            graphiql_handler("/abcd", None).await
        }
        let mut app =
            test::init_service(App::new().route("/", web::get().to(graphql_handler))).await;
        let req = test::TestRequest::get()
            .uri("/")
            .header("accept", "text/html")
            .to_request();

        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), http::StatusCode::OK);
    }

    #[actix_rt::test]
    async fn graphiql_endpoint_returns_graphiql_source() {
        async fn graphql_handler() -> Result<HttpResponse, Error> {
            graphiql_handler("/dogs-api/graphql", Some("/dogs-api/subscriptions")).await
        }
        let mut app =
            test::init_service(App::new().route("/", web::get().to(graphql_handler))).await;
        let req = test::TestRequest::get()
            .uri("/")
            .header("accept", "text/html")
            .to_request();

        let mut resp = test::call_service(&mut app, req).await;
        let body = take_response_body_string(&mut resp).await;
        assert_eq!(resp.status(), http::StatusCode::OK);
        assert_eq!(
            resp.headers().get(CONTENT_TYPE).unwrap().to_str().unwrap(),
            "text/html; charset=utf-8"
        );
        assert!(body.contains("<script>var GRAPHQL_URL = '/dogs-api/graphql';</script>"));
        assert!(body.contains(
            "<script>var GRAPHQL_SUBSCRIPTIONS_URL = '/dogs-api/subscriptions';</script>"
        ))
    }

    #[actix_rt::test]
    async fn playground_endpoint_matches() {
        async fn graphql_handler() -> Result<HttpResponse, Error> {
            playground_handler("/abcd", None).await
        }
        let mut app =
            test::init_service(App::new().route("/", web::get().to(graphql_handler))).await;
        let req = test::TestRequest::get()
            .uri("/")
            .header("accept", "text/html")
            .to_request();

        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), http::StatusCode::OK);
    }

    #[actix_rt::test]
    async fn playground_endpoint_returns_playground_source() {
        async fn graphql_handler() -> Result<HttpResponse, Error> {
            playground_handler("/dogs-api/graphql", Some("/dogs-api/subscriptions")).await
        }
        let mut app =
            test::init_service(App::new().route("/", web::get().to(graphql_handler))).await;
        let req = test::TestRequest::get()
            .uri("/")
            .header("accept", "text/html")
            .to_request();

        let mut resp = test::call_service(&mut app, req).await;
        let body = take_response_body_string(&mut resp).await;
        assert_eq!(resp.status(), http::StatusCode::OK);
        assert_eq!(
            resp.headers().get(CONTENT_TYPE).unwrap().to_str().unwrap(),
            "text/html; charset=utf-8"
        );
        assert!(body.contains("GraphQLPlayground.init(root, { endpoint: '/dogs-api/graphql', subscriptionEndpoint: '/dogs-api/subscriptions' })"));
    }

    #[actix_rt::test]
    async fn graphql_post_works_json_post() {
        let schema: Schema = RootNode::new(
            Query,
            EmptyMutation::<Database>::new(),
            EmptySubscription::<Database>::new(),
        );

        let req = test::TestRequest::post()
            .header("content-type", "application/json")
            .set_payload(
                r##"{ "variables": null, "query": "{ hero(episode: NEW_HOPE) { name } }" }"##,
            )
            .uri("/")
            .to_request();

        let mut app =
            test::init_service(App::new().data(schema).route("/", web::post().to(index))).await;

        let mut resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        assert_eq!(
            take_response_body_string(&mut resp).await,
            r#"{"data":{"hero":{"name":"R2-D2"}}}"#
        );
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "application/json",
        );
    }

    #[actix_rt::test]
    async fn graphql_get_works() {
        let schema: Schema = RootNode::new(
            Query,
            EmptyMutation::<Database>::new(),
            EmptySubscription::<Database>::new(),
        );

        let req = test::TestRequest::get()
            .header("content-type", "application/json")
            .uri("/?query=%7B%20hero%28episode%3A%20NEW_HOPE%29%20%7B%20name%20%7D%20%7D&variables=null")
            .to_request();

        let mut app =
            test::init_service(App::new().data(schema).route("/", web::get().to(index))).await;

        let mut resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        assert_eq!(
            take_response_body_string(&mut resp).await,
            r#"{"data":{"hero":{"name":"R2-D2"}}}"#
        );
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "application/json",
        );
    }

    #[actix_rt::test]
    async fn batch_request_works() {
        use juniper::{
            tests::{model::Database, schema::Query},
            EmptyMutation, EmptySubscription, RootNode,
        };

        let schema: Schema = RootNode::new(
            Query,
            EmptyMutation::<Database>::new(),
            EmptySubscription::<Database>::new(),
        );

        let req = test::TestRequest::post()
            .header("content-type", "application/json")
            .set_payload(
                r##"[
                     { "variables": null, "query": "{ hero(episode: NEW_HOPE) { name } }" },
                     { "variables": null, "query": "{ hero(episode: EMPIRE) { id name } }" }
                 ]"##,
            )
            .uri("/")
            .to_request();

        let mut app =
            test::init_service(App::new().data(schema).route("/", web::post().to(index))).await;

        let mut resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        assert_eq!(
            take_response_body_string(&mut resp).await,
            r#"[{"data":{"hero":{"name":"R2-D2"}}},{"data":{"hero":{"id":"1000","name":"Luke Skywalker"}}}]"#
        );
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "application/json",
        );
    }

    #[test]
    fn batch_request_deserialization_can_fail() {
        let json = r#"blah"#;
        let result: Result<GraphQLBatchRequest, _> = serde_json::from_str(json);

        assert!(result.is_err());
    }

    #[cfg(feature = "subscriptions")]
    #[actix_rt::test]
    async fn subscriptions() {
        use actix_web::HttpRequest;
        use actix_web_actors::ws::{Frame, Message};
        use futures::{SinkExt, Stream};
        use juniper::{DefaultScalarValue, EmptyMutation, FieldError, RootNode};
        use juniper_subscriptions::Coordinator;
        use std::{pin::Pin, time::Duration};

        pub struct Query;

        #[juniper::graphql_object(Context = Database)]
        impl Query {
            fn hello_world() -> &str {
                "Hello World!"
            }
        }
        type Schema = RootNode<'static, Query, EmptyMutation<Database>, Subscription>;
        type StringStream = Pin<Box<dyn Stream<Item = Result<String, FieldError>> + Send>>;
        type MyCoordinator = Coordinator<
            'static,
            Query,
            EmptyMutation<Database>,
            Subscription,
            Database,
            DefaultScalarValue,
        >;
        struct Subscription;

        #[derive(Clone)]
        pub struct Database;

        impl juniper::Context for Database {}

        impl Database {
            fn new() -> Self {
                Self {}
            }
        }

        #[juniper::graphql_subscription(Context = Database)]
        impl Subscription {
            async fn hello_world() -> StringStream {
                let mut counter = 0;
                let stream = tokio::time::interval(Duration::from_secs(1)).map(move |_| {
                    counter += 1;
                    if counter % 2 == 0 {
                        Ok(String::from("World!"))
                    } else {
                        Ok(String::from("Hello"))
                    }
                });

                Box::pin(stream)
            }
        }

        let schema: Schema =
            RootNode::new(Query, EmptyMutation::<Database>::new(), Subscription {});

        async fn graphql_subscriptions(
            coordinator: web::Data<MyCoordinator>,
            stream: web::Payload,
            req: HttpRequest,
        ) -> Result<HttpResponse, Error> {
            let context = Database::new();
            unsafe {
                subscriptions::graphql_subscriptions(
                    coordinator,
                    context,
                    stream,
                    req,
                    Some(subscriptions::EmptySubscriptionHandler::default()),
                )
            }
            .await
        }
        let coord = web::Data::new(juniper_subscriptions::Coordinator::new(schema));
        let mut app = test::start(move || {
            App::new()
                .app_data(coord.clone())
                .service(web::resource("/subscriptions").to(graphql_subscriptions))
        });
        let mut ws = app.ws_at("/subscriptions").await.unwrap();
        let messages_to_be_sent = vec![
            String::from(r#"{"type":"connection_init","payload":{}}"#),
            String::from(
                r#"{"id":"1","type":"start","payload":{"variables":{},"extensions":{},"operationName":"hello","query":"subscription hello {  helloWorld}"}}"#,
            ),
            String::from(r#"{"id":"1","type":"stop"}"#),
        ];
        let messages_to_be_received = vec![
            vec![
                bytes::Bytes::from(r#"{"type":"connection_ack", "payload": null }"#),
                bytes::Bytes::from(r#"{"type":"ka", "payload": null }"#),
            ],
            vec![bytes::Bytes::from(
                r#"{"type":"data","id":"1","payload":{"data":{"helloWorld":"Hello"}} }"#,
            )],
            vec![bytes::Bytes::from(
                r#"{"type":"complete","id":"1","payload":null}"#,
            )],
        ];

        for (index, msg_to_be_sent) in messages_to_be_sent.into_iter().enumerate() {
            let expected_msgs = messages_to_be_received.get(index).unwrap();
            ws.send(Message::Text(msg_to_be_sent)).await.unwrap();
            for expected_msg in expected_msgs {
                let (item, ws_stream) = ws.into_future().await;
                ws = ws_stream;
                if let Some(Ok(Frame::Text(msg))) = item {
                    assert_eq!(msg, expected_msg);
                } else {
                    assert!(false);
                }
            }
        }
    }

    pub struct TestActixWebIntegration {}

    impl HTTPIntegration for TestActixWebIntegration {
        fn get(&self, url: &str) -> TestResponse {
            let url = url.to_string();
            actix_rt::System::new("get_request").block_on(async move {
                let schema: Schema = RootNode::new(
                    Query,
                    EmptyMutation::<Database>::new(),
                    EmptySubscription::<Database>::new(),
                );
                let req = test::TestRequest::get()
                    .header("content-type", "application/json")
                    .uri(&url.clone())
                    .to_request();

                let mut app =
                    test::init_service(App::new().data(schema).route("/", web::get().to(index)))
                        .await;

                let resp = test::call_service(&mut app, req).await;
                let test_response = make_test_response(resp).await;
                test_response
            })
        }

        fn post(&self, url: &str, body: &str) -> TestResponse {
            let url = url.to_string();
            let body = body.to_string();
            actix_rt::System::new("post_request").block_on(async move {
                let schema: Schema = RootNode::new(
                    Query,
                    EmptyMutation::<Database>::new(),
                    EmptySubscription::<Database>::new(),
                );

                let req = test::TestRequest::post()
                    .header("content-type", "application/json")
                    .set_payload(body)
                    .uri(&url.clone())
                    .to_request();

                let mut app =
                    test::init_service(App::new().data(schema).route("/", web::post().to(index)))
                        .await;

                let resp = test::call_service(&mut app, req).await;
                let test_response = make_test_response(resp).await;
                test_response
            })
        }
    }

    async fn make_test_response(mut response: ServiceResponse) -> TestResponse {
        let body = take_response_body_string(&mut response).await;
        let status_code = response.status().as_u16();
        let content_type = response.headers().get(CONTENT_TYPE).unwrap();
        TestResponse {
            status_code: status_code as i32,
            body: Some(body),
            content_type: content_type.to_str().unwrap().to_string(),
        }
    }

    #[test]
    fn test_actix_web_integration() {
        run_http_test_suite(&TestActixWebIntegration {});
    }
}
