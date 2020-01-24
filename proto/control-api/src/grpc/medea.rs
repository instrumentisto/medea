/// Request of creating new Element with in element with a given FID (full ID).
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateRequest {
    /// FID (full ID) of the Element in which the provided Element will be created.
    #[prost(string, tag="1")]
    pub parent_fid: std::string::String,
    /// Spec of the created Element.
    #[prost(oneof="create_request::El", tags="2, 3, 4, 5")]
    pub el: ::std::option::Option<create_request::El>,
}
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
    pub fid: ::std::vec::Vec<std::string::String>,
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
    pub error: ::std::option::Option<Error>,
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
    pub sid: ::std::collections::HashMap<std::string::String, std::string::String>,
    /// Error of the CreateResponse.
    #[prost(message, optional, tag="2")]
    pub error: ::std::option::Option<Error>,
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
    pub elements: ::std::collections::HashMap<std::string::String, Element>,
    /// Error of the GetResponse.
    #[prost(message, optional, tag="2")]
    pub error: ::std::option::Option<Error>,
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
    pub text: std::string::String,
    /// Link to online documentation of the Error.
    ///
    /// Optional field.
    #[prost(string, tag="3")]
    pub doc: std::string::String,
    /// Full ID of Element that the Error is related to.
    /// Some Errors are not related to any Element and in such case
    /// this field is empty.
    ///
    /// Optional field.
    #[prost(string, tag="4")]
    pub element: std::string::String,
}
/// Media element which can be used in a media pipeline.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Element {
    #[prost(oneof="element::El", tags="1, 2, 3, 4")]
    pub el: ::std::option::Option<element::El>,
}
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
    pub id: std::string::String,
    /// Pipeline of this Room.
    #[prost(map="string, message", tag="2")]
    pub pipeline: ::std::collections::HashMap<std::string::String, room::Element>,
}
pub mod room {
    /// Elements which Room's pipeline can contain.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Element {
        #[prost(oneof="element::El", tags="1, 2, 3")]
        pub el: ::std::option::Option<element::El>,
    }
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
    pub id: std::string::String,
    /// Callback which fires when the Member establishes persistent connection
    /// with a media server via Client API.
    #[prost(string, tag="2")]
    pub on_join: std::string::String,
    /// Callback which fires when the Member finishes persistent connection
    /// with a media server via Client API.
    #[prost(string, tag="3")]
    pub on_leave: std::string::String,
    /// Credentials of the Member to authorize via Client API with.
    #[prost(string, tag="4")]
    pub credentials: std::string::String,
    /// Pipeline of this Member.
    #[prost(map="string, message", tag="5")]
    pub pipeline: ::std::collections::HashMap<std::string::String, member::Element>,
}
pub mod member {
    /// Elements which Member's pipeline can contain.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Element {
        #[prost(oneof="element::El", tags="1, 2")]
        pub el: ::std::option::Option<element::El>,
    }
    pub mod element {
        #[derive(Clone, PartialEq, ::prost::Oneof)]
        pub enum El {
            #[prost(message, tag="1")]
            WebrtcPlay(super::super::WebRtcPlayEndpoint),
            #[prost(message, tag="2")]
            WebrtcPub(super::super::WebRtcPublishEndpoint),
        }
    }
}
/// Media element which is able to receive media data from a client via WebRTC
/// (allows to publish media data).
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WebRtcPublishEndpoint {
    /// ID of this WebRtcPublishEndpoint.
    #[prost(string, tag="1")]
    pub id: std::string::String,
    /// P2P mode for this element.
    #[prost(enumeration="web_rtc_publish_endpoint::P2p", tag="2")]
    pub p2p: i32,
    /// Callback which fires when a client starts publishing media data.
    #[prost(string, tag="3")]
    pub on_start: std::string::String,
    /// Callback which fires when a client stops publishing media data.
    #[prost(string, tag="4")]
    pub on_stop: std::string::String,
    /// Option to relay all media through a TURN server forcibly.
    #[prost(bool, tag="5")]
    pub force_relay: bool,
}
pub mod web_rtc_publish_endpoint {
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
    pub id: std::string::String,
    /// The source to get media data from.
    #[prost(string, tag="2")]
    pub src: std::string::String,
    /// Callback which fires when a client starts playing media data
    /// from the source.
    #[prost(string, tag="3")]
    pub on_start: std::string::String,
    /// Callback which fires when a client stops playing media data
    /// from the source.
    #[prost(string, tag="4")]
    pub on_stop: std::string::String,
    /// Option to relay all media through a TURN server forcibly.
    #[prost(bool, tag="5")]
    pub force_relay: bool,
}
# [ doc = r" Generated client implementations." ] pub mod control_api_client { # ! [ allow ( unused_variables , dead_code , missing_docs ) ] use tonic :: codegen :: * ; # [ doc = " Media server's Control API service." ] pub struct ControlApiClient < T > { inner : tonic :: client :: Grpc < T > , } impl ControlApiClient < tonic :: transport :: Channel > { # [ doc = r" Attempt to create a new client by connecting to a given endpoint." ] pub async fn connect < D > ( dst : D ) -> Result < Self , tonic :: transport :: Error > where D : std :: convert :: TryInto < tonic :: transport :: Endpoint > , D :: Error : Into < StdError > , { let conn = tonic :: transport :: Endpoint :: new ( dst ) ? . connect ( ) . await ? ; Ok ( Self :: new ( conn ) ) } } impl < T > ControlApiClient < T > where T : tonic :: client :: GrpcService < tonic :: body :: BoxBody > , T :: ResponseBody : Body + HttpBody + Send + 'static , T :: Error : Into < StdError > , < T :: ResponseBody as HttpBody > :: Error : Into < StdError > + Send , { pub fn new ( inner : T ) -> Self { let inner = tonic :: client :: Grpc :: new ( inner ) ; Self { inner } } pub fn with_interceptor ( inner : T , interceptor : impl Into < tonic :: Interceptor > ) -> Self { let inner = tonic :: client :: Grpc :: with_interceptor ( inner , interceptor ) ; Self { inner } } # [ doc = " Creates new Element with a given ID." ] # [ doc = "" ] # [ doc = " Not idempotent. Errors if an Element with the same ID already exists." ] pub async fn create ( & mut self , request : impl tonic :: IntoRequest < super :: CreateRequest > , ) -> Result < tonic :: Response < super :: CreateResponse > , tonic :: Status > { self . inner . ready ( ) . await . map_err ( | e | { tonic :: Status :: new ( tonic :: Code :: Unknown , format ! ( "Service was not ready: {}" , e . into ( ) ) ) } ) ? ; let codec = tonic :: codec :: ProstCodec :: default ( ) ; let path = http :: uri :: PathAndQuery :: from_static ( "/medea.ControlApi/Create" ) ; self . inner . unary ( request . into_request ( ) , path , codec ) . await } # [ doc = " Removes Element by its ID." ] # [ doc = " Allows referring multiple Elements on the last two levels." ] # [ doc = "" ] # [ doc = " Idempotent. If no Elements with such IDs exist, then succeeds." ] pub async fn delete ( & mut self , request : impl tonic :: IntoRequest < super :: IdRequest > , ) -> Result < tonic :: Response < super :: Response > , tonic :: Status > { self . inner . ready ( ) . await . map_err ( | e | { tonic :: Status :: new ( tonic :: Code :: Unknown , format ! ( "Service was not ready: {}" , e . into ( ) ) ) } ) ? ; let codec = tonic :: codec :: ProstCodec :: default ( ) ; let path = http :: uri :: PathAndQuery :: from_static ( "/medea.ControlApi/Delete" ) ; self . inner . unary ( request . into_request ( ) , path , codec ) . await } # [ doc = " Returns Element by its ID." ] # [ doc = " Allows referring multiple Elements." ] # [ doc = " If no ID specified, returns all Elements declared." ] pub async fn get ( & mut self , request : impl tonic :: IntoRequest < super :: IdRequest > , ) -> Result < tonic :: Response < super :: GetResponse > , tonic :: Status > { self . inner . ready ( ) . await . map_err ( | e | { tonic :: Status :: new ( tonic :: Code :: Unknown , format ! ( "Service was not ready: {}" , e . into ( ) ) ) } ) ? ; let codec = tonic :: codec :: ProstCodec :: default ( ) ; let path = http :: uri :: PathAndQuery :: from_static ( "/medea.ControlApi/Get" ) ; self . inner . unary ( request . into_request ( ) , path , codec ) . await } } impl < T : Clone > Clone for ControlApiClient < T > { fn clone ( & self ) -> Self { Self { inner : self . inner . clone ( ) , } } } }# [ doc = r" Generated server implementations." ] pub mod control_api_server { # ! [ allow ( unused_variables , dead_code , missing_docs ) ] use tonic :: codegen :: * ; # [ doc = "Generated trait containing gRPC methods that should be implemented for use with ControlApiServer." ] # [ async_trait ] pub trait ControlApi : Send + Sync + 'static { # [ doc = " Creates new Element with a given ID." ] # [ doc = "" ] # [ doc = " Not idempotent. Errors if an Element with the same ID already exists." ] async fn create ( & self , request : tonic :: Request < super :: CreateRequest > ) -> Result < tonic :: Response < super :: CreateResponse > , tonic :: Status > ; # [ doc = " Removes Element by its ID." ] # [ doc = " Allows referring multiple Elements on the last two levels." ] # [ doc = "" ] # [ doc = " Idempotent. If no Elements with such IDs exist, then succeeds." ] async fn delete ( & self , request : tonic :: Request < super :: IdRequest > ) -> Result < tonic :: Response < super :: Response > , tonic :: Status > ; # [ doc = " Returns Element by its ID." ] # [ doc = " Allows referring multiple Elements." ] # [ doc = " If no ID specified, returns all Elements declared." ] async fn get ( & self , request : tonic :: Request < super :: IdRequest > ) -> Result < tonic :: Response < super :: GetResponse > , tonic :: Status > ; } # [ doc = " Media server's Control API service." ] # [ derive ( Debug ) ] # [ doc ( hidden ) ] pub struct ControlApiServer < T : ControlApi > { inner : _Inner < T > , } struct _Inner < T > ( Arc < T > , Option < tonic :: Interceptor > ) ; impl < T : ControlApi > ControlApiServer < T > { pub fn new ( inner : T ) -> Self { let inner = Arc :: new ( inner ) ; let inner = _Inner ( inner , None ) ; Self { inner } } pub fn with_interceptor ( inner : T , interceptor : impl Into < tonic :: Interceptor > ) -> Self { let inner = Arc :: new ( inner ) ; let inner = _Inner ( inner , Some ( interceptor . into ( ) ) ) ; Self { inner } } } impl < T : ControlApi > Service < http :: Request < HyperBody >> for ControlApiServer < T > { type Response = http :: Response < tonic :: body :: BoxBody > ; type Error = Never ; type Future = BoxFuture < Self :: Response , Self :: Error > ; fn poll_ready ( & mut self , _cx : & mut Context < '_ > ) -> Poll < Result < ( ) , Self :: Error >> { Poll :: Ready ( Ok ( ( ) ) ) } fn call ( & mut self , req : http :: Request < HyperBody > ) -> Self :: Future { let inner = self . inner . clone ( ) ; match req . uri ( ) . path ( ) { "/medea.ControlApi/Create" => { struct CreateSvc < T : ControlApi > ( pub Arc < T > ) ; impl < T : ControlApi > tonic :: server :: UnaryService < super :: CreateRequest > for CreateSvc < T > { type Response = super :: CreateResponse ; type Future = BoxFuture < tonic :: Response < Self :: Response > , tonic :: Status > ; fn call ( & mut self , request : tonic :: Request < super :: CreateRequest > ) -> Self :: Future { let inner = self . 0 . clone ( ) ; let fut = async move { inner . create ( request ) . await } ; Box :: pin ( fut ) } } let inner = self . inner . clone ( ) ; let fut = async move { let interceptor = inner . 1 . clone ( ) ; let inner = inner . 0 ; let method = CreateSvc ( inner ) ; let codec = tonic :: codec :: ProstCodec :: default ( ) ; let mut grpc = if let Some ( interceptor ) = interceptor { tonic :: server :: Grpc :: with_interceptor ( codec , interceptor ) } else { tonic :: server :: Grpc :: new ( codec ) } ; let res = grpc . unary ( method , req ) . await ; Ok ( res ) } ; Box :: pin ( fut ) } "/medea.ControlApi/Delete" => { struct DeleteSvc < T : ControlApi > ( pub Arc < T > ) ; impl < T : ControlApi > tonic :: server :: UnaryService < super :: IdRequest > for DeleteSvc < T > { type Response = super :: Response ; type Future = BoxFuture < tonic :: Response < Self :: Response > , tonic :: Status > ; fn call ( & mut self , request : tonic :: Request < super :: IdRequest > ) -> Self :: Future { let inner = self . 0 . clone ( ) ; let fut = async move { inner . delete ( request ) . await } ; Box :: pin ( fut ) } } let inner = self . inner . clone ( ) ; let fut = async move { let interceptor = inner . 1 . clone ( ) ; let inner = inner . 0 ; let method = DeleteSvc ( inner ) ; let codec = tonic :: codec :: ProstCodec :: default ( ) ; let mut grpc = if let Some ( interceptor ) = interceptor { tonic :: server :: Grpc :: with_interceptor ( codec , interceptor ) } else { tonic :: server :: Grpc :: new ( codec ) } ; let res = grpc . unary ( method , req ) . await ; Ok ( res ) } ; Box :: pin ( fut ) } "/medea.ControlApi/Get" => { struct GetSvc < T : ControlApi > ( pub Arc < T > ) ; impl < T : ControlApi > tonic :: server :: UnaryService < super :: IdRequest > for GetSvc < T > { type Response = super :: GetResponse ; type Future = BoxFuture < tonic :: Response < Self :: Response > , tonic :: Status > ; fn call ( & mut self , request : tonic :: Request < super :: IdRequest > ) -> Self :: Future { let inner = self . 0 . clone ( ) ; let fut = async move { inner . get ( request ) . await } ; Box :: pin ( fut ) } } let inner = self . inner . clone ( ) ; let fut = async move { let interceptor = inner . 1 . clone ( ) ; let inner = inner . 0 ; let method = GetSvc ( inner ) ; let codec = tonic :: codec :: ProstCodec :: default ( ) ; let mut grpc = if let Some ( interceptor ) = interceptor { tonic :: server :: Grpc :: with_interceptor ( codec , interceptor ) } else { tonic :: server :: Grpc :: new ( codec ) } ; let res = grpc . unary ( method , req ) . await ; Ok ( res ) } ; Box :: pin ( fut ) } _ => Box :: pin ( async move { Ok ( http :: Response :: builder ( ) . status ( 200 ) . header ( "grpc-status" , "12" ) . body ( tonic :: body :: BoxBody :: empty ( ) ) . unwrap ( ) ) } ) , } } } impl < T : ControlApi > Clone for ControlApiServer < T > { fn clone ( & self ) -> Self { let inner = self . inner . clone ( ) ; Self { inner } } } impl < T : ControlApi > Clone for _Inner < T > { fn clone ( & self ) -> Self { Self ( self . 0 . clone ( ) , self . 1 . clone ( ) ) } } impl < T : std :: fmt :: Debug > std :: fmt :: Debug for _Inner < T > { fn fmt ( & self , f : & mut std :: fmt :: Formatter < '_ > ) -> std :: fmt :: Result { write ! ( f , "{:?}" , self . 0 ) } } impl < T : ControlApi > tonic :: transport :: NamedService for ControlApiServer < T > { const NAME : & 'static str = "medea.ControlApi" ; } }