export class MockNavigator {
  constructor() {
    let self = this;

    this.getUserMediaInvocations = 0;
    this.getDisplayMediaInvocations = 0;

    this._enumerateDevices = window.navigator.mediaDevices.enumerateDevices;
    this._getUserMedia = window.navigator.mediaDevices.getUserMedia;
    this._getDisplayMedia = window.navigator.mediaDevices.getDisplayMedia;


    window.navigator.mediaDevices.getUserMedia = async function(arg) {
      self.getUserMediaInvocations++;
      return await self._getUserMedia.call(
        window.navigator.mediaDevices,
        arg
      );
    }

    window.navigator.mediaDevices.getDisplayMedia = async function(arg) {
      self.getDisplayMediaInvocations++;
      return await self._getDisplayMedia.call(
        window.navigator.mediaDevices,
        arg
      );
    }
  }

  set errorEnumerateDevices(err) {
    window.navigator.mediaDevices.enumerateDevices = async function() {throw err}
  }

  set errorGetUserMedia(err) {
    window.navigator.mediaDevices.getUserMedia = async function() {throw err}
  }

  set errorGetDisplayMedia(err) {
    window.navigator.mediaDevices.getDisplayMedia = async function() {throw err}
  }

  get getUserMediaRequestsCount() {
    return this.getUserMediaInvocations;
  }

  get getDisplayMediaRequestsCount() {
    return this.getDisplayMediaInvocations;
  }

  set setUserMediaReturns(stream) {
    window.navigator.mediaDevices.getUserMedia = async function() {return stream};
  }

  set setDisplayMediaReturns(stream) {
    window.navigator.mediaDevices.getDisplayMedia = async function() {return stream};
  }

  stop() {
    window.navigator.mediaDevices.getUserMedia = this._getUserMedia;
    window.navigator.mediaDevices.getDisplayMedia = this._getDisplayMedia;
    window.navigator.mediaDevices.enumerateDevices = this._enumerateDevices;
  }
}
