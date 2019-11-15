export class MockNavigator {
  set errorEnumerateDevices(err) {
    this._enumerateDevices = window.navigator.mediaDevices.enumerateDevices;
    window.navigator.mediaDevices.enumerateDevices = async function() {throw err}
  }

  set errorGetUserMedia(err) {
    this._getUserMedia = window.navigator.mediaDevices.getUserMedia;
    window.navigator.mediaDevices.getUserMedia = async function() {throw err}
  }

  stop() {
    if (this._getUserMedia) {
      window.navigator.mediaDevices.getUserMedia = this._getUserMedia;
    }
    if (this._enumerateDevices) {
      window.navigator.mediaDevices.enumerateDevices = this._enumerateDevices;
    }
  }
}
