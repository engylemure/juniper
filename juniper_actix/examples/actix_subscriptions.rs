#![deny(warnings)]

use actix_cors::Cors;
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use futures::Stream;
use juniper::{
    tests::{model::Database, schema::Query},
    DefaultScalarValue, EmptyMutation, FieldError, RootNode,
};
use juniper_actix::{
    graphiql_handler as gqli_handler, graphql_handler, playground_handler as play_handler,
    subscriptions::{graphql_subscriptions as sub_handler, EmptySubscriptionHandler},
};
use juniper_subscriptions::Coordinator;
use std::{pin::Pin, time::Duration};

type Schema = RootNode<'static, Query, EmptyMutation<Database>, Subscription>;
type MyCoordinator = Coordinator<
    'static,
    Query,
    EmptyMutation<Database>,
    Subscription,
    Database,
    DefaultScalarValue,
>;

type StringStream = Pin<Box<dyn Stream<Item = Result<String, FieldError>> + Send>>;

struct Subscription;

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

fn schema() -> Schema {
    Schema::new(Query {}, EmptyMutation::new(), Subscription {})
}

async fn graphiql_handler() -> Result<HttpResponse, Error> {
    gqli_handler("/", Some("/subscriptions")).await
}
async fn playground_handler() -> Result<HttpResponse, Error> {
    play_handler("/", Some("/subscriptions")).await
}

async fn graphql(
    req: actix_web::HttpRequest,
    payload: actix_web::web::Payload,
    schema: web::Data<Schema>,
) -> Result<HttpResponse, Error> {
    let context = Database::new();
    graphql_handler(&schema, &context, req, payload).await
}

async fn graphql_subscriptions(
    coordinator: web::Data<MyCoordinator>,
    stream: web::Payload,
    req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let context = Database::new();
    let handler: Option<EmptySubscriptionHandler> = None;
    unsafe { sub_handler(coordinator, context, stream, req, handler) }.await
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    ::std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();
    let server = HttpServer::new(move || {
        App::new()
            .data(schema())
            .data(juniper_subscriptions::Coordinator::new(schema()))
            .wrap(middleware::Compress::default())
            .wrap(middleware::Logger::default())
            .wrap(
                Cors::new()
                    .allowed_methods(vec!["POST", "GET"])
                    .supports_credentials()
                    .max_age(3600)
                    .finish(),
            )
            .service(
                web::resource("/")
                    .route(web::post().to(graphql))
                    .route(web::get().to(graphql)),
            )
            .service(web::resource("/playground").route(web::get().to(playground_handler)))
            .service(web::resource("/graphiql").route(web::get().to(graphiql_handler)))
            .service(web::resource("/subscriptions").to(graphql_subscriptions))
    });
    server.bind("127.0.0.1:8080").unwrap().run().await
}
