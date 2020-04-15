use serde_derive::{Deserialize, Serialize};
/// Enum of Subscription Protocol Message Types over WS
/// to know more access
/// https://github.com/apollographql/subscriptions-transport-ws/blob/master/PROTOCOL.md
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GraphQLOverWebSocketMessage {
    /// Client -> Server
    /// Client sends this message after plain websocket connection to start the communication
    /// with the server
    #[serde(rename = "connection_init")]
    ConnectionInit,
    /// Server -> Client
    /// The server may responses with this message to the GQL_CONNECTION_INIT from client,
    /// indicates the server accepted the connection.
    #[serde(rename = "connection_ack")]
    ConnectionAck,
    /// Server -> Client
    /// The server may responses with this message to the GQL_CONNECTION_INIT from client,
    /// indicates the server rejected the connection.
    #[serde(rename = "connection_error")]
    ConnectionError,
    /// Server -> Client
    /// Server message that should be sent right after each GQL_CONNECTION_ACK processed
    /// and then periodically to keep the client connection alive.
    #[serde(rename = "ka")]
    ConnectionKeepAlive,
    /// Client -> Server
    /// Client sends this message to terminate the connection.
    #[serde(rename = "connection_terminate")]
    ConnectionTerminate,
    /// Client -> Server
    /// Client sends this message to execute GraphQL operation
    #[serde(rename = "start")]
    Start,
    /// Server -> Client
    /// The server sends this message to transfer the GraphQL execution result from the
    /// server to the client, this message is a response for GQL_START message.
    #[serde(rename = "data")]
    Data,
    /// Server -> Client
    /// Server sends this message upon a failing operation, before the GraphQL execution,
    /// usually due to GraphQL validation errors (resolver errors are part of GQL_DATA message, and will be added as errors array)
    #[serde(rename = "error")]
    Error,
    /// Server -> Client
    /// Server sends this message to indicate that a GraphQL operation is done,
    /// and no more data will arrive for the specific operation.
    #[serde(rename = "complete")]
    Complete,
    /// Client -> Server
    /// Client sends this message in order to stop a running GraphQL operation execution
    /// (for example: unsubscribe)
    #[serde(rename = "stop")]
    Stop,
}

/// Empty SubscriptionLifeCycleHandler over WS
pub enum SubscriptionState<'a, Context>
where
    Context: Send + Sync,
{
    /// The Subscription is at the init of the connection with the client after the
    /// server receives the GQL_CONNECTION_INIT message.
    OnConnection(Option<String>, &'a mut Context),
    /// The Subscription is at the start of a operation after the GQL_START message is
    /// is received.
    OnOperation(&'a mut Context),
    /// The subscription is on the end of a operation before sending the GQL_COMPLETE
    /// message to the client.
    OnOperationComplete(&'a Context),
    /// The Subscription is terminating the connection with the client.
    OnDisconnect(&'a Context),
}

/// Trait based on the SubscriptionServer
/// https://www.apollographql.com/docs/graphql-subscriptions/lifecycle-events/
pub trait SubscriptionStateHandler<Context, E>
where
    Context: Send + Sync,
    E: std::error::Error,
{
    /// This function is called when the state of the Subscription changes
    /// with the actual state.
    fn handle(&self, _state: SubscriptionState<Context>) -> Result<(), E> {
        Ok(())
    }
}

/// A Empty Subscription Handler
#[derive(Default)]
pub struct EmptySubscriptionHandler;

impl<Context> SubscriptionStateHandler<Context, std::io::Error> for EmptySubscriptionHandler where
    Context: Send + Sync
{
}
