/*!

# juniper_actix

This repository contains the [actix][actix] web server integration for
[Juniper][Juniper], a [GraphQL][GraphQL] implementation for Rust.

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

*/

#![deny(missing_docs)]
#![deny(warnings)]
#![doc(html_root_url = "https://docs.rs/juniper_actix/0.1.0")]

// use futures::{FutureExt as _};
use actix_web::{web, Error, HttpResponse};
use juniper::{
    graphiql::graphiql_source, http::playground::playground_source, DefaultScalarValue, InputValue,
    ScalarValue,
};
use serde::Deserialize;

/// Enum for handling batch requests
#[derive(Debug, serde_derive::Deserialize, PartialEq)]
#[serde(untagged)]
#[serde(bound = "InputValue<S>: Deserialize<'de>")]
pub enum GraphQLBatchRequest<S = DefaultScalarValue>
where
    S: ScalarValue,
{
    /// Single Request
    Single(juniper::http::GraphQLRequest<S>),
    /// Batch Request
    Batch(Vec<juniper::http::GraphQLRequest<S>>),
}

#[allow(dead_code)]
impl<S> GraphQLBatchRequest<S>
where
    S: ScalarValue,
{
    /// Execute synchronous
    pub fn execute_sync<'a, CtxT, QueryT, MutationT, SubscriptionT>(
        &'a self,
        root_node: &'a juniper::RootNode<QueryT, MutationT, SubscriptionT, S>,
        context: &CtxT,
    ) -> GraphQLBatchResponse<'a, S>
    where
        QueryT: juniper::GraphQLType<S, Context = CtxT>,
        MutationT: juniper::GraphQLType<S, Context = CtxT>,
        SubscriptionT: juniper::GraphQLType<S, Context = CtxT>,
        SubscriptionT::TypeInfo: Send + Sync,
        CtxT: Send + Sync,
    {
        match *self {
            GraphQLBatchRequest::Single(ref request) => {
                GraphQLBatchResponse::Single(request.execute_sync(root_node, context))
            }
            GraphQLBatchRequest::Batch(ref requests) => GraphQLBatchResponse::Batch(
                requests
                    .iter()
                    .map(|request| request.execute_sync(root_node, context))
                    .collect(),
            ),
        }
    }

    /// Execute asynchronous
    pub async fn execute<'a, CtxT, QueryT, MutationT, SubscriptionT>(
        &'a self,
        root_node: &'a juniper::RootNode<'a, QueryT, MutationT, SubscriptionT, S>,
        context: &'a CtxT,
    ) -> GraphQLBatchResponse<'a, S>
    where
        QueryT: juniper::GraphQLTypeAsync<S, Context = CtxT> + Send + Sync,
        QueryT::TypeInfo: Send + Sync,
        MutationT: juniper::GraphQLTypeAsync<S, Context = CtxT> + Send + Sync,
        MutationT::TypeInfo: Send + Sync,
        SubscriptionT: juniper::GraphQLSubscriptionType<S, Context = CtxT> + Send + Sync,
        SubscriptionT::TypeInfo: Send + Sync,
        CtxT: Send + Sync,
        S: Send + Sync,
    {
        match *self {
            GraphQLBatchRequest::Single(ref request) => {
                let res = request.execute(root_node, context).await;
                GraphQLBatchResponse::Single(res)
            }
            GraphQLBatchRequest::Batch(ref requests) => {
                let futures = requests
                    .iter()
                    .map(|request| request.execute(root_node, context))
                    .collect::<Vec<_>>();
                let responses = futures::future::join_all(futures).await;

                GraphQLBatchResponse::Batch(responses)
            }
        }
    }
}

/// Enum for the batch response
#[derive(serde_derive::Serialize)]
#[serde(untagged)]
pub enum GraphQLBatchResponse<'a, S = DefaultScalarValue>
where
    S: ScalarValue,
{
    /// When is a single response
    Single(juniper::http::GraphQLResponse<'a, S>),
    /// When is a batch response
    Batch(Vec<juniper::http::GraphQLResponse<'a, S>>),
}

#[allow(dead_code)]
impl<'a, S> GraphQLBatchResponse<'a, S>
where
    S: ScalarValue,
{
    fn is_ok(&self) -> bool {
        match self {
            GraphQLBatchResponse::Single(res) => res.is_ok(),
            GraphQLBatchResponse::Batch(reses) => reses.iter().all(|res| res.is_ok()),
        }
    }
}

/// Actix GraphQL Handler for GET requests
pub async fn get_graphql_handler<Query, Mutation, Subscription, Context, S>(
    schema: &juniper::RootNode<'static, Query, Mutation, Subscription, S>,
    context: &Context,
    req: web::Query<GraphQLBatchRequest<S>>,
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
    let gql_batch_response = req.execute(schema, context).await;

    let gql_response = serde_json::to_string(&gql_batch_response)?;
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(gql_response))
}
/// Actix GraphQL Handler for POST requests
pub async fn post_graphql_handler<Query, Mutation, Subscription, Context, S>(
    schema: &juniper::RootNode<'static, Query, Mutation, Subscription, S>,
    context: &Context,
    req: web::Json<GraphQLBatchRequest<S>>,
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
    let gql_batch_response = req.execute(schema, context).await;
    let gql_response = serde_json::to_string(&gql_batch_response)?;
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(gql_response))
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
///          .route("/", web::get().to(|| graphiql_handler("/graphql")));
/// ```
#[allow(dead_code)]
pub async fn graphiql_handler(graphql_endpoint_url: &str) -> Result<HttpResponse, Error> {
    let html = graphiql_source(graphql_endpoint_url);
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

/// Subscriptions Module
#[cfg(feature = "subscriptions")]
pub mod subscriptions {
    use actix::{Actor, ActorContext, ActorFuture, AsyncContext, StreamHandler, WrapFuture};
    use actix_web::{error::PayloadError, web, web::Bytes, Error, HttpRequest, HttpResponse};
    use actix_web_actors::{
        ws,
        ws::{handshake_with_protocols, WebsocketContext},
    };
    use futures::{Stream, StreamExt};
    use juniper::{http::GraphQLRequest, InputValue, ScalarValue, SubscriptionCoordinator};
    use juniper_subscriptions::{message_types::*, Coordinator};
    use serde::{Deserialize, Serialize};
    use std::{
        collections::HashMap,
        error::Error as StdError,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
    };
    use tokio::time::Duration;

    fn start<Query, Mutation, Subscription, Context, S, FunStart, T>(
        actor: GraphQLWSSession<Query, Mutation, Subscription, Context, S, FunStart>,
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
        FunStart: std::marker::Unpin + FnMut(&mut Context, String) -> Result<(), String> + 'static,
    {
        let mut res = handshake_with_protocols(req, &["graphql-ws"])?;
        Ok(res.streaming(WebsocketContext::create(actor, stream)))
    }
    /// Since this implementation makes usage of the unsafe keyword i will consider this as unsafe for now.
    pub async unsafe fn graphql_subscriptions<Query, Mutation, Subscription, Context, S, FunStart>(
        coordinator: web::Data<Coordinator<'static, Query, Mutation, Subscription, Context, S>>,
        context: Context,
        stream: web::Payload,
        req: HttpRequest,
        on_start: FunStart,
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
        FunStart: std::marker::Unpin + FnMut(&mut Context, String) -> Result<(), String> + 'static,
    {
        start(
            GraphQLWSSession {
                coordinator: coordinator.into_inner(),
                graphql_context: context,
                is_closed: Arc::new(AtomicBool::new(false)),
                has_started: Arc::new(AtomicBool::new(false)),
                map_req_id_to_is_closed: Arc::new(Mutex::new(HashMap::new())),
                on_start,
            },
            &req,
            stream,
        )
    }

    struct GraphQLWSSession<Query, Mutation, Subscription, Context, S, FunStart>
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
        FunStart: std::marker::Unpin + FnMut(&mut Context, String) -> Result<(), String> + 'static,
    {
        pub is_closed: Arc<AtomicBool>,
        pub map_req_id_to_is_closed: Arc<Mutex<HashMap<String, bool>>>,
        pub has_started: Arc<AtomicBool>,
        pub graphql_context: Context,
        pub coordinator: Arc<Coordinator<'static, Query, Mutation, Subscription, Context, S>>,
        pub on_start: FunStart,
    }

    impl<Query, Mutation, Subscription, Context, S, FunStart> Actor
        for GraphQLWSSession<Query, Mutation, Subscription, Context, S, FunStart>
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
        FunStart: std::marker::Unpin + FnMut(&mut Context, String) -> Result<(), String> + 'static,
    {
        type Context = ws::WebsocketContext<
            GraphQLWSSession<Query, Mutation, Subscription, Context, S, FunStart>,
        >;
    }

    #[allow(dead_code)]
    impl<Query, Mutation, Subscription, Context, S, FunStart>
        GraphQLWSSession<Query, Mutation, Subscription, Context, S, FunStart>
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
        FunStart: std::marker::Unpin + FnMut(&mut Context, String) -> Result<(), String> + 'static,
    {
        fn gql_connection_ack() -> String {
            format!(r#"{{"type":"{}", "payload": null }}"#, GQL_CONNECTION_ACK)
        }

        fn gql_connection_ka() -> String {
            format!(
                r#"{{"type":"{}", "payload": null }}"#,
                GQL_CONNECTION_KEEP_ALIVE
            )
        }

        fn gql_connection_error() -> String {
            format!(r#"{{"type":"{}", "payload": null }}"#, GQL_CONNECTION_ERROR)
        }
        fn gql_error<T: StdError + Serialize>(request_id: &String, err: T) -> String {
            format!(
                r#"{{"type":"{}","id":"{}","payload":{}}}"#,
                GQL_ERROR,
                request_id,
                serde_json::ser::to_string(&err)
                    .unwrap_or("Error deserializing GraphQLError".to_owned())
            )
        }

        fn gql_data(request_id: &String, response_text: String) -> String {
            format!(
                r#"{{"type":"{}","id":"{}","payload":{} }}"#,
                GQL_DATA, request_id, response_text
            )
        }

        fn gql_complete(request_id: &String) -> String {
            format!(
                r#"{{"type":"{}","id":"{}","payload":null}}"#,
                GQL_COMPLETE, request_id
            )
        }

        fn starting_handle(
            result: (
                GraphQLRequest<S>,
                String,
                Context,
                Arc<Coordinator<'static, Query, Mutation, Subscription, Context, S>>,
                Arc<AtomicBool>,
            ),
            actor: &mut Self,
            ctx: &mut ws::WebsocketContext<Self>,
        ) -> actix::fut::FutureWrap<impl futures::Future<Output = ()>, Self> {
            let ctx: *mut ws::WebsocketContext<Self> = ctx;
            let (req, req_id, gql_context, coord, close_signal) = result;
            Self::handle_subscription(
                req,
                gql_context,
                req_id,
                coord,
                ctx,
                close_signal,
                actor.map_req_id_to_is_closed.clone(),
            )
            .into_actor(actor)
        }

        async fn handle_subscription(
            req: GraphQLRequest<S>,
            graphql_context: Context,
            request_id: String,
            coord: Arc<Coordinator<'static, Query, Mutation, Subscription, Context, S>>,
            ctx: *mut ws::WebsocketContext<Self>,
            got_close_signal: Arc<AtomicBool>,
            map_req_id_to_is_closed: Arc<Mutex<HashMap<String, bool>>>,
        ) {
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
            {
                let mut map = map_req_id_to_is_closed.lock().unwrap();
                map.insert(request_id.clone(), false);
            }
            while let Some(response) = values_stream.next().await {
                let request_id = request_id.clone();
                let closed = got_close_signal.load(Ordering::Relaxed);
                if !closed {
                    let map = map_req_id_to_is_closed.lock().unwrap();
                    let is_req_closed = map.get(&request_id).unwrap();
                    if !is_req_closed {
                        let response_text = serde_json::to_string(&response)
                            .unwrap_or("Error deserializing respone".to_owned());
                        ctx.text(Self::gql_data(&request_id, response_text));
                    } else {
                        break;
                    }
                } else {
                    ctx.stop();
                    break;
                }
            }
        }
    }

    impl<Query, Mutation, Subscription, Context, S, FunStart>
        StreamHandler<Result<ws::Message, ws::ProtocolError>>
        for GraphQLWSSession<Query, Mutation, Subscription, Context, S, FunStart>
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
        FunStart: std::marker::Unpin + FnMut(&mut Context, String) -> Result<(), String> + 'static,
    {
        fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
            let msg = match msg {
                Err(_) => {
                    ctx.stop();
                    return;
                }
                Ok(msg) => msg,
            };
            let coordinator = self.coordinator.clone();
            let context = self.graphql_context.clone();
            let got_close_signal = self.is_closed.clone();
            let has_started = self.has_started.clone();
            let has_started_value = has_started.load(Ordering::Relaxed);
            match msg {
                ws::Message::Text(text) => {
                    let m = text.trim();
                    let request: WsPayload<S> = serde_json::from_str(m).expect("Invalid WsPayload");
                    match request.type_name.as_str() {
                        GQL_CONNECTION_INIT => {
                            match (self.on_start)(&mut self.graphql_context, String::from(m)) {
                                Ok(_) => {
                                    ctx.text(Self::gql_connection_ack());
                                    ctx.text(Self::gql_connection_ka());
                                    has_started.store(true, Ordering::Relaxed);
                                    ctx.run_interval(Duration::from_secs(15), |actor, ctx| {
                                        let no_request = {
                                            let map = actor.map_req_id_to_is_closed.lock().unwrap();
                                            map.values().fold(true, |acc, val| acc && *val)
                                        };
                                        if no_request {
                                            actor.is_closed.store(true, Ordering::Relaxed);
                                            ctx.stop();
                                        } else if !actor.is_closed.load(Ordering::Relaxed) {
                                            ctx.text(Self::gql_connection_ka());
                                        }
                                    });
                                }
                                Err(_err) => ctx.text(Self::gql_connection_error()),
                            }
                        }
                        GQL_START if has_started_value => {
                            let payload = request.payload.expect("Could not deserialize payload");
                            let request_id = request.id.unwrap_or("1".to_owned());
                            let graphql_request = GraphQLRequest::<_>::new(
                                payload.query.expect("Could not deserialize query"),
                                None,
                                payload.variables,
                            );
                            {
                                let future = async move {
                                    (
                                        graphql_request,
                                        request_id,
                                        context,
                                        coordinator,
                                        got_close_signal,
                                    )
                                }
                                .into_actor(self)
                                .then(Self::starting_handle);
                                ctx.spawn(future);
                            }
                        }
                        GQL_STOP if has_started_value => {
                            let request_id = request.id.unwrap_or("1".to_owned());
                            let close_message = format!(
                                r#"{{"type":"{}","id":"{}","payload":null}}"#,
                                GQL_COMPLETE, request_id
                            );
                            ctx.text(close_message);
                            let mut map = self.map_req_id_to_is_closed.lock().unwrap();
                            map.insert(request_id, true);
                        }
                        GQL_CONNECTION_TERMINATE if has_started_value => {
                            got_close_signal.store(true, Ordering::Relaxed);
                            ctx.stop();
                        }
                        _ if !has_started_value => {
                            got_close_signal.store(true, Ordering::Relaxed);
                            ctx.stop();
                        }
                        _ => {}
                    }
                }
                ws::Message::Binary(_) => println!("Unexpected binary"),
                ws::Message::Close(_) => {
                    got_close_signal.store(true, Ordering::Relaxed);
                    ctx.stop();
                }
                ws::Message::Continuation(_) => {
                    got_close_signal.store(true, Ordering::Relaxed);
                    ctx.stop();
                }
                _ => (),
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
        type_name: String,
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
        let response_body = response_body.unwrap();
        String::from_utf8(response_body).unwrap()
    }

    async fn index(
        req: web::Json<GraphQLBatchRequest<DefaultScalarValue>>,
        schema: web::Data<Schema>,
    ) -> Result<HttpResponse, Error> {
        let context = Database::new();
        post_graphql_handler(&schema, &context, req).await
    }

    async fn index_get(
        req: web::Query<GraphQLBatchRequest<DefaultScalarValue>>,
        schema: web::Data<Schema>,
    ) -> Result<HttpResponse, Error> {
        let context = Database::new();
        get_graphql_handler(&schema, &context, req).await
    }

    #[actix_rt::test]
    async fn graphiql_response_does_not_panic() {
        let result = graphiql_handler("/abcd").await;
        assert!(result.is_ok())
    }

    #[actix_rt::test]
    async fn graphiql_endpoint_matches() {
        async fn graphql_handler() -> Result<HttpResponse, Error> {
            graphiql_handler("/abcd").await
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
            graphiql_handler("/dogs-api/graphql").await
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
        assert!(body.contains("<script>var GRAPHQL_URL = '/dogs-api/graphql';</script>"))
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
            test::init_service(App::new().data(schema).route("/", web::get().to(index_get))).await;

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
}
