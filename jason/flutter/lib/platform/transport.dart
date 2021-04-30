import 'utils/ffi.dart' as ffi;
import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'package:web_socket_channel/io.dart';

void linkWebSocketFunctions() {
  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>("register_WebSocketRpcTransport__new")(
      Pointer.fromFunction<Handle Function(Pointer<Utf8>)>(newWs)
  );

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>("register_WebSocketRpcTransport__on_message")(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(listenWs)
  );

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>("register_WebSocketRpcTransport__send")(
      Pointer.fromFunction<Void Function(Handle, Pointer<Utf8>)>(sendWsMsg)
  );

  ffi.dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>("register_WebSocketRpcTransport__on_close")(
      Pointer.fromFunction<Void Function(Handle, Pointer)>(listenClose)
  );
}

Object newWs(Pointer<Utf8> addr) {
  return IOWebSocketChannel.connect(Uri.parse(addr.toDartString()));
}

final _callMessageListenerDart _callMessageListener = ffi.dl
    .lookupFunction<_callMessageListenerC, _callMessageListenerDart>('WsMessageListener__call');
typedef _callMessageListenerC = Pointer<Utf8> Function(Pointer, Pointer<Utf8>);
typedef _callMessageListenerDart = Pointer<Utf8> Function(Pointer, Pointer<Utf8>);

void listenWs(Object ws, Pointer listener) {
  if (ws is IOWebSocketChannel) {
    ws.stream.listen((msg) {
      if (msg is String) {
        _callMessageListener(listener, msg.toNativeUtf8());
      }
    });
  }
}

void listenClose(Object ws, Pointer listener) {
  if (ws is IOWebSocketChannel) {
    ws.stream.listen((msg) {
      if (msg is String) {
        _callMessageListener(listener, msg.toNativeUtf8());
      }
    });
  }
}

void sendWsMsg(Object ws, Pointer<Utf8> msg) {
  if (ws is IOWebSocketChannel) {
    ws.sink.add(msg.toDartString());
  }
}