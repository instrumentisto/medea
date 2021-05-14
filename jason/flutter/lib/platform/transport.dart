import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'package:medea_jason/util/nullable_pointer.dart';
import 'package:web_socket_channel/io.dart';
import '../jason.dart';

void registerFunctions(DynamicLibrary dl) {
  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          "register_WebSocketRpcTransport__new")(
      Pointer.fromFunction<Handle Function(Pointer<Utf8>)>(newWs));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          "register_WebSocketRpcTransport__on_message")(
      Pointer.fromFunction<Void Function(Handle, Handle)>(listenWs));

  dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
          "register_WebSocketRpcTransport__send")(
      Pointer.fromFunction<Void Function(Handle, Pointer<Utf8>)>(sendWsMsg));

  // dl.lookupFunction<Void Function(Pointer), void Function(Pointer)>(
  //         "register_WebSocketRpcTransport__on_close")(
  //     Pointer.fromFunction<Void Function(Handle, Pointer)>(listenClose));
}

Object newWs(Pointer<Utf8> addr) {
  return IOWebSocketChannel.connect(Uri.parse(addr.toDartString()));
}

final _callMessageListenerDart _callMessageListener = dl
    .lookupFunction<_callMessageListenerC, _callMessageListenerDart>(
        'StringCallback__call');
typedef _callMessageListenerC = Pointer<Utf8> Function(Pointer, Pointer<Utf8>);
typedef _callMessageListenerDart = Pointer<Utf8> Function(
    Pointer, Pointer<Utf8>);

void listenWs(Object ws, Object callback) {
  if (ws is IOWebSocketChannel) {
    ws.stream.listen((msg) {
      if (msg is String) {
        var cb = callback as Function(String);
        cb(msg);
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
