/// Request of creating new Element with in element with a given FID (full ID).
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateRequest {
    /// FID (full ID) of the Element in which the provided Element will be created.
    #[prost(string, tag="1")]
    pub parent_fid: ::prost::alloc::string::String,
    /// Spec of the created Element.
    #[prost(oneof="create_request::El", tags="2, 3, 4, 5")]
    pub el: ::core::option::Option<create_request::El>,
}
/// Nested message and enum types in `CreateRequest`.
pub mod create_request {
    /// Spec of the created Element.
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum El {
        #[prost(message, tag="2")]
        Member(super::Member),
        #[prost(message, tag="3")]
        Room(super::Room),
        #[prost(message, tag="4")]
        WebrtcPlay(super::WebRtcPlayEndpoint),
        #[prost(message, tag="5")]
        WebrtcPub(super::WebRtcPublishEndpoint),
    }
}
/// Request with many FIDs (full IDs) of Elements.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IdRequest {
    /// List of Elements FIDs.
    #[prost(string, repeated, tag="1")]
    pub fid: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// Request of applying a spec to Element with the given FID (full ID).
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ApplyRequest {
    /// FID (full ID) of the Element to apply the given spec to.
    #[prost(string, tag="1")]
    pub parent_fid: ::prost::alloc::string::String,
    /// Spec of the Element to be applied.
    #[prost(oneof="apply_request::El", tags="2, 3, 4, 5")]
    pub el: ::core::option::Option<apply_request::El>,
}
/// Nested message and enum types in `ApplyRequest`.
pub mod apply_request {
    /// Spec of the Element to be applied.
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum El {
        #[prost(message, tag="2")]
        Member(super::Member),
        #[prost(message, tag="3")]
        Room(super::Room),
        #[prost(message, tag="4")]
        WebrtcPlay(super::WebRtcPlayEndpoint),
        #[prost(message, tag="5")]
        WebrtcPub(super::WebRtcPublishEndpoint),
    }
}
/// Response which doesn't return anything on successful result,
/// but is fallible with an Error.
///
/// If operation fails then an Error will be returned.
/// The response is considered successful only if it does not contain Error.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Response {
    /// Error of the Response.
    #[prost(message, optional, tag="1")]
    pub error: ::core::option::Option<Error>,
}
/// Response of Create RPC method.
///
/// If operation fails then an Error will be returned.
/// The response is considered successful only if it does not contain Error.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateResponse {
    /// Hashmap with IDs (key) and URIs (value) of Elements, which should be used
    /// by clients to connect to a media server via Client API.
    ///
    /// Returned only if CreateResponse is successful.
    #[prost(map="string, string", tag="1")]
    pub sid: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    /// Error of the CreateResponse.
    #[prost(message, optional, tag="2")]
    pub error: ::core::option::Option<Error>,
}
/// Response of Get RPC method.
///
/// If operation fails then an Error will be returned.
/// The response is considered successful only if it does not contain Error.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetResponse {
    /// Hashmap with IDs (key) and specs (value) of the requested Elements.
    ///
    /// Returned only if GetResponse is successful.
    #[prost(map="string, message", tag="1")]
    pub elements: ::std::collections::HashMap<::prost::alloc::string::String, Element>,
    /// Error of the GetResponse.
    #[prost(message, optional, tag="2")]
    pub error: ::core::option::Option<Error>,
}
/// Error of failed request.
///
/// If the Error is not returned then request is considered as successful.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Error {
    /// Concrete unique code of the Error.
    #[prost(uint32, tag="1")]
    pub code: u32,
    /// Human-readable text description of the Error.
    #[prost(string, tag="2")]
    pub text: ::prost::alloc::string::String,
    /// Link to online documentation of the Error.
    ///
    /// Optional field.
    #[prost(string, tag="3")]
    pub doc: ::prost::alloc::string::String,
    /// Full ID of Element that the Error is related to.
    /// Some Errors are not related to any Element and in such case
    /// this field is empty.
    ///
    /// Optional field.
    #[prost(string, tag="4")]
    pub element: ::prost::alloc::string::String,
}
/// Media element which can be used in a media pipeline.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Element {
    #[prost(oneof="element::El", tags="1, 2, 3, 4")]
    pub el: ::core::option::Option<element::El>,
}
/// Nested message and enum types in `Element`.
pub mod element {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum El {
        #[prost(message, tag="1")]
        Member(super::Member),
        #[prost(message, tag="2")]
        Room(super::Room),
        #[prost(message, tag="3")]
        WebrtcPlay(super::WebRtcPlayEndpoint),
        #[prost(message, tag="4")]
        WebrtcPub(super::WebRtcPublishEndpoint),
    }
}
/// Media element which represents a single space where multiple Members can
/// interact with each other.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Room {
    /// ID of this Room.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    /// Pipeline of this Room.
    #[prost(map="string, message", tag="2")]
    pub pipeline: ::std::collections::HashMap<::prost::alloc::string::String, room::Element>,
}
/// Nested message and enum types in `Room`.
pub mod room {
    /// Elements which Room's pipeline can contain.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Element {
        #[prost(oneof="element::El", tags="1, 2, 3")]
        pub el: ::core::option::Option<element::El>,
    }
    /// Nested message and enum types in `Element`.
    pub mod element {
        #[derive(Clone, PartialEq, ::prost::Oneof)]
        pub enum El {
            #[prost(message, tag="1")]
            Member(super::super::Member),
            #[prost(message, tag="2")]
            WebrtcPlay(super::super::WebRtcPlayEndpoint),
            #[prost(message, tag="3")]
            WebrtcPub(super::super::WebRtcPublishEndpoint),
        }
    }
}
/// Media element which represents a client authorized to participate
/// in a some bigger media pipeline.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Member {
    /// ID of this Member.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    /// Callback which fires when the Member establishes persistent connection
    /// with a media server via Client API.
    #[prost(string, tag="2")]
    pub on_join: ::prost::alloc::string::String,
    /// Callback which fires when the Member finishes persistent connection
    /// with a media server via Client API.
    #[prost(string, tag="3")]
    pub on_leave: ::prost::alloc::string::String,
    /// Timeout of receiving heartbeat messages from the Member via Client API.
    /// Once reached, the Member is considered being idle.
    #[prost(message, optional, tag="6")]
    pub idle_timeout: ::core::option::Option<::prost_types::Duration>,
    /// Timeout of the Member reconnecting via Client API.
    /// Once reached, the Member is considered disconnected.
    #[prost(message, optional, tag="7")]
    pub reconnect_timeout: ::core::option::Option<::prost_types::Duration>,
    /// Interval of sending pings from a media server to the Member via Client API.
    #[prost(message, optional, tag="8")]
    pub ping_interval: ::core::option::Option<::prost_types::Duration>,
    /// Pipeline of this Member.
    #[prost(map="string, message", tag="9")]
    pub pipeline: ::std::collections::HashMap<::prost::alloc::string::String, member::Element>,
    /// Credentials of the Member to authorize via Client API with.
    ///
    /// Plain and hashed credentials are supported. If no credentials provided,
    /// then random plain string will be generated. If no authentication is
    /// required then empty plain string can be used.
    ///
    /// Hashed variant only supports Argon2 hash at the moment.
    /// Member sid won't contain token if hashed credentials are used, so token
    /// query parameter should be appended manually.
    #[prost(oneof="member::Credentials", tags="4, 5")]
    pub credentials: ::core::option::Option<member::Credentials>,
}
/// Nested message and enum types in `Member`.
pub mod member {
    /// Elements which Member's pipeline can contain.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Element {
        #[prost(oneof="element::El", tags="1, 2")]
        pub el: ::core::option::Option<element::El>,
    }
    /// Nested message and enum types in `Element`.
    pub mod element {
        #[derive(Clone, PartialEq, ::prost::Oneof)]
        pub enum El {
            #[prost(message, tag="1")]
            WebrtcPlay(super::super::WebRtcPlayEndpoint),
            #[prost(message, tag="2")]
            WebrtcPub(super::super::WebRtcPublishEndpoint),
        }
    }
    /// Credentials of the Member to authorize via Client API with.
    ///
    /// Plain and hashed credentials are supported. If no credentials provided,
    /// then random plain string will be generated. If no authentication is
    /// required then empty plain string can be used.
    ///
    /// Hashed variant only supports Argon2 hash at the moment.
    /// Member sid won't contain token if hashed credentials are used, so token
    /// query parameter should be appended manually.
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Credentials {
        /// Argon2 hash of credentials.
        #[prost(string, tag="4")]
        Hash(::prost::alloc::string::String),
        /// Plain text credentials.
        #[prost(string, tag="5")]
        Plain(::prost::alloc::string::String),
    }
}
/// Media element which is able to receive media data from a client via WebRTC
/// (allows to publish media data).
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WebRtcPublishEndpoint {
    /// ID of this WebRtcPublishEndpoint.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    /// P2P mode for this element.
    #[prost(enumeration="web_rtc_publish_endpoint::P2p", tag="2")]
    pub p2p: i32,
    /// Callback which fires when a client starts publishing media data.
    #[prost(string, tag="3")]
    pub on_start: ::prost::alloc::string::String,
    /// Callback which fires when a client stops publishing media data.
    #[prost(string, tag="4")]
    pub on_stop: ::prost::alloc::string::String,
    /// Option to relay all media through a TURN server forcibly.
    #[prost(bool, tag="5")]
    pub force_relay: bool,
    /// Settings for the audio media type of this element.
    #[prost(message, optional, tag="6")]
    pub audio_settings: ::core::option::Option<web_rtc_publish_endpoint::AudioSettings>,
    /// Settings for the video media type of this element.
    #[prost(message, optional, tag="7")]
    pub video_settings: ::core::option::Option<web_rtc_publish_endpoint::VideoSettings>,
}
/// Nested message and enum types in `WebRtcPublishEndpoint`.
pub mod web_rtc_publish_endpoint {
    /// Audio media type settings of WebRtcPublishEndpoint.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct AudioSettings {
        /// Policy to publish audio media type with.
        #[prost(enumeration="PublishPolicy", tag="1")]
        pub publish_policy: i32,
    }
    /// Video media type settings of WebRtcPublishEndpoint.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct VideoSettings {
        /// Policy to publish video media type with.
        #[prost(enumeration="PublishPolicy", tag="1")]
        pub publish_policy: i32,
    }
    /// Policy of how the video or audio media type can be published in
    /// WebRtcPublishEndpoint.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum PublishPolicy {
        /// Media type MAY be published.
        ///
        /// Media server will try to initialize publishing, but won't produce any
        /// errors if user application fails to (or chooses not to) acquire a
        /// required media track. Media server will approve user requests to stop and
        /// to restart publishing the specified media type.
        Optional = 0,
        /// Media type MUST be published.
        ///
        /// Media server will try to initialize publishing, and if a required media
        /// track couldn't be acquired, then an error will be thrown. Media server
        /// will deny all requests to stop publishing.
        Required = 1,
        /// Media type MUST not be published.
        ///
        /// Media server will not try to initialize publishing.
        Disabled = 2,
    }
    /// P2P mode of WebRTC interaction.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum P2p {
        /// Always send media data through a media server.
        Never = 0,
        /// Send media data peer-to-peer directly if it's possible,
        /// otherwise through a media server.
        IfPossible = 1,
        /// Send media data peer-to-peer only without a media server.
        Always = 2,
    }
}
/// Media element which is able to play media data for a client via WebRTC.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WebRtcPlayEndpoint {
    /// ID of this WebRtcPlayEndpoint.
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    /// The source to get media data from.
    #[prost(string, tag="2")]
    pub src: ::prost::alloc::string::String,
    /// Callback which fires when a client starts playing media data
    /// from the source.
    #[prost(string, tag="3")]
    pub on_start: ::prost::alloc::string::String,
    /// Callback which fires when a client stops playing media data
    /// from the source.
    #[prost(string, tag="4")]
    pub on_stop: ::prost::alloc::string::String,
    /// Option to relay all media through a TURN server forcibly.
    #[prost(bool, tag="5")]
    pub force_relay: bool,
}
# [doc = r" Generated client implementations."] pub mod control_api_client { # ! [allow (unused_variables , dead_code , missing_docs , clippy :: let_unit_value ,)] use tonic :: codegen :: * ; # [doc = " Media server's Control API service."] # [derive (Debug , Clone)] pub struct ControlApiClient < T > { inner : tonic :: client :: Grpc < T > , } impl ControlApiClient < tonic :: transport :: Channel > { # [doc = r" Attempt to create a new client by connecting to a given endpoint."] pub async fn connect < D > (dst : D) -> Result < Self , tonic :: transport :: Error > where D : std :: convert :: TryInto < tonic :: transport :: Endpoint > , D :: Error : Into < StdError > , { let conn = tonic :: transport :: Endpoint :: new (dst) ? . connect () . await ? ; Ok (Self :: new (conn)) } } impl < T > ControlApiClient < T > where T : tonic :: client :: GrpcService < tonic :: body :: BoxBody > , T :: ResponseBody : Body + Send + Sync + 'static , T :: Error : Into < StdError > , < T :: ResponseBody as Body > :: Error : Into < StdError > + Send , { pub fn new (inner : T) -> Self { let inner = tonic :: client :: Grpc :: new (inner) ; Self { inner } } pub fn with_interceptor < F > (inner : T , interceptor : F) -> ControlApiClient < InterceptedService < T , F >> where F : FnMut (tonic :: Request < () >) -> Result < tonic :: Request < () > , tonic :: Status > , T : tonic :: codegen :: Service < http :: Request < tonic :: body :: BoxBody > , Response = http :: Response << T as tonic :: client :: GrpcService < tonic :: body :: BoxBody >> :: ResponseBody > > , < T as tonic :: codegen :: Service < http :: Request < tonic :: body :: BoxBody >> > :: Error : Into < StdError > + Send + Sync , { ControlApiClient :: new (InterceptedService :: new (inner , interceptor)) } # [doc = r" Compress requests with `gzip`."] # [doc = r""] # [doc = r" This requires the server to support it otherwise it might respond with an"] # [doc = r" error."] pub fn send_gzip (mut self) -> Self { self . inner = self . inner . send_gzip () ; self } # [doc = r" Enable decompressing responses with `gzip`."] pub fn accept_gzip (mut self) -> Self { self . inner = self . inner . accept_gzip () ; self } # [doc = " Creates new Element with a given ID."] # [doc = ""] # [doc = " Not idempotent. Errors if an Element with the same ID already exists."] pub async fn create (& mut self , request : impl tonic :: IntoRequest < super :: CreateRequest > ,) -> Result < tonic :: Response < super :: CreateResponse > , tonic :: Status > { self . inner . ready () . await . map_err (| e | { tonic :: Status :: new (tonic :: Code :: Unknown , format ! ("Service was not ready: {}" , e . into ())) }) ? ; let codec = tonic :: codec :: ProstCodec :: default () ; let path = http :: uri :: PathAndQuery :: from_static ("/api.ControlApi/Create") ; self . inner . unary (request . into_request () , path , codec) . await } # [doc = " Removes Element by its ID."] # [doc = " Allows referring multiple Elements on the last two levels."] # [doc = ""] # [doc = " Idempotent. If no Elements with such IDs exist, then succeeds."] pub async fn delete (& mut self , request : impl tonic :: IntoRequest < super :: IdRequest > ,) -> Result < tonic :: Response < super :: Response > , tonic :: Status > { self . inner . ready () . await . map_err (| e | { tonic :: Status :: new (tonic :: Code :: Unknown , format ! ("Service was not ready: {}" , e . into ())) }) ? ; let codec = tonic :: codec :: ProstCodec :: default () ; let path = http :: uri :: PathAndQuery :: from_static ("/api.ControlApi/Delete") ; self . inner . unary (request . into_request () , path , codec) . await } # [doc = " Returns Element by its ID."] # [doc = " Allows referring multiple Elements."] # [doc = " If no ID specified, returns all Elements declared."] pub async fn get (& mut self , request : impl tonic :: IntoRequest < super :: IdRequest > ,) -> Result < tonic :: Response < super :: GetResponse > , tonic :: Status > { self . inner . ready () . await . map_err (| e | { tonic :: Status :: new (tonic :: Code :: Unknown , format ! ("Service was not ready: {}" , e . into ())) }) ? ; let codec = tonic :: codec :: ProstCodec :: default () ; let path = http :: uri :: PathAndQuery :: from_static ("/api.ControlApi/Get") ; self . inner . unary (request . into_request () , path , codec) . await } # [doc = " Applies the given spec to Element by its ID."] # [doc = ""] # [doc = " Idempotent. If no Element with such ID exists, then it will be created,"] # [doc = " otherwise it will be reconfigured. Elements that exist, but are not"] # [doc = " specified in the provided spec will be removed."] pub async fn apply (& mut self , request : impl tonic :: IntoRequest < super :: ApplyRequest > ,) -> Result < tonic :: Response < super :: CreateResponse > , tonic :: Status > { self . inner . ready () . await . map_err (| e | { tonic :: Status :: new (tonic :: Code :: Unknown , format ! ("Service was not ready: {}" , e . into ())) }) ? ; let codec = tonic :: codec :: ProstCodec :: default () ; let path = http :: uri :: PathAndQuery :: from_static ("/api.ControlApi/Apply") ; self . inner . unary (request . into_request () , path , codec) . await } } }# [doc = r" Generated server implementations."] pub mod control_api_server { # ! [allow (unused_variables , dead_code , missing_docs , clippy :: let_unit_value ,)] use tonic :: codegen :: * ; # [doc = "Generated trait containing gRPC methods that should be implemented for use with ControlApiServer."] # [async_trait] pub trait ControlApi : Send + Sync + 'static { # [doc = " Creates new Element with a given ID."] # [doc = ""] # [doc = " Not idempotent. Errors if an Element with the same ID already exists."] async fn create (& self , request : tonic :: Request < super :: CreateRequest >) -> Result < tonic :: Response < super :: CreateResponse > , tonic :: Status > ; # [doc = " Removes Element by its ID."] # [doc = " Allows referring multiple Elements on the last two levels."] # [doc = ""] # [doc = " Idempotent. If no Elements with such IDs exist, then succeeds."] async fn delete (& self , request : tonic :: Request < super :: IdRequest >) -> Result < tonic :: Response < super :: Response > , tonic :: Status > ; # [doc = " Returns Element by its ID."] # [doc = " Allows referring multiple Elements."] # [doc = " If no ID specified, returns all Elements declared."] async fn get (& self , request : tonic :: Request < super :: IdRequest >) -> Result < tonic :: Response < super :: GetResponse > , tonic :: Status > ; # [doc = " Applies the given spec to Element by its ID."] # [doc = ""] # [doc = " Idempotent. If no Element with such ID exists, then it will be created,"] # [doc = " otherwise it will be reconfigured. Elements that exist, but are not"] # [doc = " specified in the provided spec will be removed."] async fn apply (& self , request : tonic :: Request < super :: ApplyRequest >) -> Result < tonic :: Response < super :: CreateResponse > , tonic :: Status > ; } # [doc = " Media server's Control API service."] # [derive (Debug)] pub struct ControlApiServer < T : ControlApi > { inner : _Inner < T > , accept_compression_encodings : () , send_compression_encodings : () , } struct _Inner < T > (Arc < T >) ; impl < T : ControlApi > ControlApiServer < T > { pub fn new (inner : T) -> Self { let inner = Arc :: new (inner) ; let inner = _Inner (inner) ; Self { inner , accept_compression_encodings : Default :: default () , send_compression_encodings : Default :: default () , } } pub fn with_interceptor < F > (inner : T , interceptor : F) -> InterceptedService < Self , F > where F : FnMut (tonic :: Request < () >) -> Result < tonic :: Request < () > , tonic :: Status > , { InterceptedService :: new (Self :: new (inner) , interceptor) } } impl < T , B > tonic :: codegen :: Service < http :: Request < B >> for ControlApiServer < T > where T : ControlApi , B : Body + Send + Sync + 'static , B :: Error : Into < StdError > + Send + 'static , { type Response = http :: Response < tonic :: body :: BoxBody > ; type Error = Never ; type Future = BoxFuture < Self :: Response , Self :: Error > ; fn poll_ready (& mut self , _cx : & mut Context < '_ >) -> Poll < Result < () , Self :: Error >> { Poll :: Ready (Ok (())) } fn call (& mut self , req : http :: Request < B >) -> Self :: Future { let inner = self . inner . clone () ; match req . uri () . path () { "/api.ControlApi/Create" => { # [allow (non_camel_case_types)] struct CreateSvc < T : ControlApi > (pub Arc < T >) ; impl < T : ControlApi > tonic :: server :: UnaryService < super :: CreateRequest > for CreateSvc < T > { type Response = super :: CreateResponse ; type Future = BoxFuture < tonic :: Response < Self :: Response > , tonic :: Status > ; fn call (& mut self , request : tonic :: Request < super :: CreateRequest >) -> Self :: Future { let inner = self . 0 . clone () ; let fut = async move { (* inner) . create (request) . await } ; Box :: pin (fut) } } let accept_compression_encodings = self . accept_compression_encodings ; let send_compression_encodings = self . send_compression_encodings ; let inner = self . inner . clone () ; let fut = async move { let inner = inner . 0 ; let method = CreateSvc (inner) ; let codec = tonic :: codec :: ProstCodec :: default () ; let mut grpc = tonic :: server :: Grpc :: new (codec) . apply_compression_config (accept_compression_encodings , send_compression_encodings) ; let res = grpc . unary (method , req) . await ; Ok (res) } ; Box :: pin (fut) } "/api.ControlApi/Delete" => { # [allow (non_camel_case_types)] struct DeleteSvc < T : ControlApi > (pub Arc < T >) ; impl < T : ControlApi > tonic :: server :: UnaryService < super :: IdRequest > for DeleteSvc < T > { type Response = super :: Response ; type Future = BoxFuture < tonic :: Response < Self :: Response > , tonic :: Status > ; fn call (& mut self , request : tonic :: Request < super :: IdRequest >) -> Self :: Future { let inner = self . 0 . clone () ; let fut = async move { (* inner) . delete (request) . await } ; Box :: pin (fut) } } let accept_compression_encodings = self . accept_compression_encodings ; let send_compression_encodings = self . send_compression_encodings ; let inner = self . inner . clone () ; let fut = async move { let inner = inner . 0 ; let method = DeleteSvc (inner) ; let codec = tonic :: codec :: ProstCodec :: default () ; let mut grpc = tonic :: server :: Grpc :: new (codec) . apply_compression_config (accept_compression_encodings , send_compression_encodings) ; let res = grpc . unary (method , req) . await ; Ok (res) } ; Box :: pin (fut) } "/api.ControlApi/Get" => { # [allow (non_camel_case_types)] struct GetSvc < T : ControlApi > (pub Arc < T >) ; impl < T : ControlApi > tonic :: server :: UnaryService < super :: IdRequest > for GetSvc < T > { type Response = super :: GetResponse ; type Future = BoxFuture < tonic :: Response < Self :: Response > , tonic :: Status > ; fn call (& mut self , request : tonic :: Request < super :: IdRequest >) -> Self :: Future { let inner = self . 0 . clone () ; let fut = async move { (* inner) . get (request) . await } ; Box :: pin (fut) } } let accept_compression_encodings = self . accept_compression_encodings ; let send_compression_encodings = self . send_compression_encodings ; let inner = self . inner . clone () ; let fut = async move { let inner = inner . 0 ; let method = GetSvc (inner) ; let codec = tonic :: codec :: ProstCodec :: default () ; let mut grpc = tonic :: server :: Grpc :: new (codec) . apply_compression_config (accept_compression_encodings , send_compression_encodings) ; let res = grpc . unary (method , req) . await ; Ok (res) } ; Box :: pin (fut) } "/api.ControlApi/Apply" => { # [allow (non_camel_case_types)] struct ApplySvc < T : ControlApi > (pub Arc < T >) ; impl < T : ControlApi > tonic :: server :: UnaryService < super :: ApplyRequest > for ApplySvc < T > { type Response = super :: CreateResponse ; type Future = BoxFuture < tonic :: Response < Self :: Response > , tonic :: Status > ; fn call (& mut self , request : tonic :: Request < super :: ApplyRequest >) -> Self :: Future { let inner = self . 0 . clone () ; let fut = async move { (* inner) . apply (request) . await } ; Box :: pin (fut) } } let accept_compression_encodings = self . accept_compression_encodings ; let send_compression_encodings = self . send_compression_encodings ; let inner = self . inner . clone () ; let fut = async move { let inner = inner . 0 ; let method = ApplySvc (inner) ; let codec = tonic :: codec :: ProstCodec :: default () ; let mut grpc = tonic :: server :: Grpc :: new (codec) . apply_compression_config (accept_compression_encodings , send_compression_encodings) ; let res = grpc . unary (method , req) . await ; Ok (res) } ; Box :: pin (fut) } _ => Box :: pin (async move { Ok (http :: Response :: builder () . status (200) . header ("grpc-status" , "12") . header ("content-type" , "application/grpc") . body (empty_body ()) . unwrap ()) }) , } } } impl < T : ControlApi > Clone for ControlApiServer < T > { fn clone (& self) -> Self { let inner = self . inner . clone () ; Self { inner , accept_compression_encodings : self . accept_compression_encodings , send_compression_encodings : self . send_compression_encodings , } } } impl < T : ControlApi > Clone for _Inner < T > { fn clone (& self) -> Self { Self (self . 0 . clone ()) } } impl < T : std :: fmt :: Debug > std :: fmt :: Debug for _Inner < T > { fn fmt (& self , f : & mut std :: fmt :: Formatter < '_ >) -> std :: fmt :: Result { write ! (f , "{:?}" , self . 0) } } impl < T : ControlApi > tonic :: transport :: NamedService for ControlApiServer < T > { const NAME : & 'static str = "api.ControlApi" ; } }