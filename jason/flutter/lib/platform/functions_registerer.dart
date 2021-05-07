import 'dart:ffi';

import 'constraints.dart' as constraints;
import 'ice_candidate.dart' as ice_candidate;
import 'input_device_info.dart' as input_device_info;
import 'media_devices.dart' as media_devices;
import 'media_track.dart' as media_track;
import 'peer_connection.dart' as peer_connection;
import 'transceiver.dart' as transceiver;
import 'transport.dart' as transport;
import 'utils/functions_registerer.dart' as utils;

void registerFunctions(DynamicLibrary dl) {
  constraints.registerFunctions(dl);
  ice_candidate.registerFunctions(dl);
  input_device_info.registerFunctions(dl);
  media_devices.registerFunctions(dl);
  media_track.registerFunctions(dl);
  peer_connection.registerFunctions(dl);
  transceiver.registerFunctions(dl);
  transport.registerFunctions(dl);
  utils.registerFunctions(dl);
}
