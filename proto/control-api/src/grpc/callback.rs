/// Request with a fired callback event and some meta information.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Request {
    /// FID (Full ID) of the element that event was occurred with.
    #[prost(string, tag="1")]
    pub fid: ::prost::alloc::string::String,
    /// Time of event occurring.
    #[prost(string, tag="2")]
    pub at: ::prost::alloc::string::String,
    /// Occurred callback event.
    #[prost(oneof="request::Event", tags="3, 4")]
    pub event: ::core::option::Option<request::Event>,
}
/// Nested message and enum types in `Request`.
pub mod request {
    /// Occurred callback event.
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Event {
        #[prost(message, tag="3")]
        OnJoin(super::OnJoin),
        #[prost(message, tag="4")]
        OnLeave(super::OnLeave),
    }
}
/// Empty response of the Callback service.
///
/// We don't use 'google.protobuf.Empty' to be able to add
/// some fields (if necessary) in the future.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Response {
}
/// Event that fires when Member joins a Room.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OnJoin {
}
/// Event that fires when Member leaves its Room.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OnLeave {
    /// Reason of why Member leaves the Room.
    #[prost(enumeration="on_leave::Reason", tag="1")]
    pub reason: i32,
}
/// Nested message and enum types in `OnLeave`.
pub mod on_leave {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Reason {
        /// Member was normally disconnected.
        Disconnected = 0,
        /// Connection with Member was lost.
        LostConnection = 1,
        /// Member was forcibly disconnected by server.
        Kicked = 2,
        /// Medea media server is shutting down.
        ServerShutdown = 3,
    }
}
# [doc = r" Generated client implementations."] pub mod callback_client { # ! [allow (unused_variables , dead_code , missing_docs)] use tonic :: codegen :: * ; # [doc = " Service for receiving callbacks from Medea media server."] pub struct CallbackClient < T > { inner : tonic :: client :: Grpc < T > , } impl CallbackClient < tonic :: transport :: Channel > { # [doc = r" Attempt to create a new client by connecting to a given endpoint."] pub async fn connect < D > (dst : D) -> Result < Self , tonic :: transport :: Error > where D : std :: convert :: TryInto < tonic :: transport :: Endpoint > , D :: Error : Into < StdError > , { let conn = tonic :: transport :: Endpoint :: new (dst) ? . connect () . await ? ; Ok (Self :: new (conn)) } } impl < T > CallbackClient < T > where T : tonic :: client :: GrpcService < tonic :: body :: BoxBody > , T :: ResponseBody : Body + HttpBody + Send + 'static , T :: Error : Into < StdError > , < T :: ResponseBody as HttpBody > :: Error : Into < StdError > + Send , { pub fn new (inner : T) -> Self { let inner = tonic :: client :: Grpc :: new (inner) ; Self { inner } } pub fn with_interceptor (inner : T , interceptor : impl Into < tonic :: Interceptor >) -> Self { let inner = tonic :: client :: Grpc :: with_interceptor (inner , interceptor) ; Self { inner } } # [doc = "/ Fires when a certain callback event happens on Medea media server."] pub async fn on_event (& mut self , request : impl tonic :: IntoRequest < super :: Request > ,) -> Result < tonic :: Response < super :: Response > , tonic :: Status > { self . inner . ready () . await . map_err (| e | { tonic :: Status :: new (tonic :: Code :: Unknown , format ! ("Service was not ready: {}" , e . into ())) }) ? ; let codec = tonic :: codec :: ProstCodec :: default () ; let path = http :: uri :: PathAndQuery :: from_static ("/callback.Callback/OnEvent") ; self . inner . unary (request . into_request () , path , codec) . await } } impl < T : Clone > Clone for CallbackClient < T > { fn clone (& self) -> Self { Self { inner : self . inner . clone () , } } } impl < T > std :: fmt :: Debug for CallbackClient < T > { fn fmt (& self , f : & mut std :: fmt :: Formatter < '_ >) -> std :: fmt :: Result { write ! (f , "CallbackClient {{ ... }}") } } }# [doc = r" Generated server implementations."] pub mod callback_server { # ! [allow (unused_variables , dead_code , missing_docs)] use tonic :: codegen :: * ; # [doc = "Generated trait containing gRPC methods that should be implemented for use with CallbackServer."] # [async_trait] pub trait Callback : Send + Sync + 'static { # [doc = "/ Fires when a certain callback event happens on Medea media server."] async fn on_event (& self , request : tonic :: Request < super :: Request >) -> Result < tonic :: Response < super :: Response > , tonic :: Status > ; } # [doc = " Service for receiving callbacks from Medea media server."] # [derive (Debug)] pub struct CallbackServer < T : Callback > { inner : _Inner < T > , } struct _Inner < T > (Arc < T > , Option < tonic :: Interceptor >) ; impl < T : Callback > CallbackServer < T > { pub fn new (inner : T) -> Self { let inner = Arc :: new (inner) ; let inner = _Inner (inner , None) ; Self { inner } } pub fn with_interceptor (inner : T , interceptor : impl Into < tonic :: Interceptor >) -> Self { let inner = Arc :: new (inner) ; let inner = _Inner (inner , Some (interceptor . into ())) ; Self { inner } } } impl < T , B > Service < http :: Request < B >> for CallbackServer < T > where T : Callback , B : HttpBody + Send + Sync + 'static , B :: Error : Into < StdError > + Send + 'static , { type Response = http :: Response < tonic :: body :: BoxBody > ; type Error = Never ; type Future = BoxFuture < Self :: Response , Self :: Error > ; fn poll_ready (& mut self , _cx : & mut Context < '_ >) -> Poll < Result < () , Self :: Error >> { Poll :: Ready (Ok (())) } fn call (& mut self , req : http :: Request < B >) -> Self :: Future { let inner = self . inner . clone () ; match req . uri () . path () { "/callback.Callback/OnEvent" => { # [allow (non_camel_case_types)] struct OnEventSvc < T : Callback > (pub Arc < T >) ; impl < T : Callback > tonic :: server :: UnaryService < super :: Request > for OnEventSvc < T > { type Response = super :: Response ; type Future = BoxFuture < tonic :: Response < Self :: Response > , tonic :: Status > ; fn call (& mut self , request : tonic :: Request < super :: Request >) -> Self :: Future { let inner = self . 0 . clone () ; let fut = async move { (* inner) . on_event (request) . await } ; Box :: pin (fut) } } let inner = self . inner . clone () ; let fut = async move { let interceptor = inner . 1 . clone () ; let inner = inner . 0 ; let method = OnEventSvc (inner) ; let codec = tonic :: codec :: ProstCodec :: default () ; let mut grpc = if let Some (interceptor) = interceptor { tonic :: server :: Grpc :: with_interceptor (codec , interceptor) } else { tonic :: server :: Grpc :: new (codec) } ; let res = grpc . unary (method , req) . await ; Ok (res) } ; Box :: pin (fut) } _ => Box :: pin (async move { Ok (http :: Response :: builder () . status (200) . header ("grpc-status" , "12") . header ("content-type" , "application/grpc") . body (tonic :: body :: BoxBody :: empty ()) . unwrap ()) }) , } } } impl < T : Callback > Clone for CallbackServer < T > { fn clone (& self) -> Self { let inner = self . inner . clone () ; Self { inner } } } impl < T : Callback > Clone for _Inner < T > { fn clone (& self) -> Self { Self (self . 0 . clone () , self . 1 . clone ()) } } impl < T : std :: fmt :: Debug > std :: fmt :: Debug for _Inner < T > { fn fmt (& self , f : & mut std :: fmt :: Formatter < '_ >) -> std :: fmt :: Result { write ! (f , "{:?}" , self . 0) } } impl < T : Callback > tonic :: transport :: NamedService for CallbackServer < T > { const NAME : & 'static str = "callback.Callback" ; } }
