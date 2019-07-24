context('Pub<=>Pub video call', () => {
  /**
   * This function deletes room used in this context's tests from medea.
   */
  function deleteTestRoom() {
    cy.request({
      url: 'http://localhost:8000/pub-pub-e2e-call',
      method: 'DELETE',
    });
  }

  beforeEach(() => {
    deleteTestRoom();
    cy.request({
      url: 'http://localhost:8000/pub-pub-e2e-call',
      method: 'POST',
      body: {
        pipeline: {
          caller: {
            kind: 'Member',
            credentials: 'test',
            pipeline: {
              publish: {
                kind: 'WebRtcPublishEndpoint',
                spec: {
                  p2p: 'Always'
                }
              },
              play: {
                kind: 'WebRtcPlayEndpoint',
                spec: {
                  src: 'local://pub-pub-e2e-call/responder/publish',
                }
              }
            }
          },
          responder: {
            kind: 'Member',
            credentials: 'test',
            pipeline: {
              publish: {
                kind: 'WebRtcPublishEndpoint',
                spec: {
                  p2p: 'Always',
                }
              },
              play: {
                kind: 'WebRtcPlayEndpoint',
                spec: {
                  src: 'local://pub-pub-e2e-call/caller/publish',
                }
              }
            }
          }
        }
      }
    });

    cy.visit('http://localhost:8082');
  });

  afterEach(() => {
    deleteTestRoom();
  });

  it('open video call and works', () => {
    /**
     * Promise which start PubPubVideoCall.
     *
     * Note that returned promise is Cypress.Promise because Cypress don't work
     * with vanilla Promises.
     *
     * This function require window root object because it is only way
     * because this is the only way to call a function in the test application.
     *
     * This promise resolving when video call started.
     * This promise rejecting when some error occured in startPubPubVideoCall.
     * @param win root window object
     */
    function startVideoCall(win) {
      return new Cypress.Promise((resolve, reject) => {
        win.startPubPubVideoCall()
          .catch((err) => {
            reject(err)
          })
          .then((response) => {
            resolve(response)
          })
      })
    }

    /**
     * Takes array of RTCStatsReport and count "outbound-rtp" and "inbound-rtp"
     * for all RTCStatsReport. If "outbound-rtp"'s "packetsSent" or "inbound-rtp"'s
     * "packetsReceived" < 5 then test failed.
     * @param stats array of RTCStatsReports
     */
    function checkStats(stats) {
      let outboundPackets = 0;
      let inboundPackets = 0;
      stats.forEach(resp => {
        resp.forEach(report => {
          if (report.type === 'outbound-rtp') {
            outboundPackets += report.packetsSent;
          } else if (report.type === 'inbound-rtp') {
            inboundPackets += report.packetsReceived;
          }
        });
      });
      expect(outboundPackets).to.be.greaterThan(5);
      expect(inboundPackets).to.be.greaterThan(5);
    }

    /**
     * Return difference between two arrays.
     *
     * In this test it's used for comparing images received from partner.
     *
     * @param o first array
     * @param n second array
     * @returns {number} number of how arrays are different
     */
    function diff(o, n) {
      let objO = {},
        objN = {};
      for (let i = 0; i < o.length; i++) {
        objO[o[i]] = 1;
      }
      for (let i = 0; i < n.length; i++) {
        objN[n[i]] = 1;
      }
      let added = 0;
      let removed = 0;

      for (let i in objO) {
        if (i in objN) {
          delete objN[i];
        } else {
          removed += 1;
        }
      }
      for (let i in objN) {
        added += 1;
      }

      return added + removed
    }

    /**
     * Get two images from provided video element with some small interval
     * and check that they are different.
     *
     * Test will fail if difference between this two images are less than 50.
     *
     * Use for testing that video which we receiving from partner are not static.
     *
     * @param videoEl video element
     */
    function checkVideoDiff(videoEl) {
      let canvas = document.createElement('canvas');
      canvas.height = videoEl.videoHeight / 2;
      canvas.width = videoEl.videoWidth / 2;

      let context = canvas.getContext('2d');
      context.drawImage(videoEl, canvas.width, canvas.height, canvas.width, canvas.height);
      let imgEl = document.createElement('img');
      imgEl.src = canvas.toDataURL();
      let firstData = context.getImageData(0, 0, canvas.width, canvas.height);

      context.drawImage(videoEl, 0, 0, canvas.width, canvas.height);
      imgEl.src = canvas.toDataURL();
      let secondData = context.getImageData(0, 0, canvas.width, canvas.height);

      let dataDiff = diff(firstData.data, secondData.data);

      expect(dataDiff).to.be.greaterThan(50);
    }

    cy.window()
      .then((win) => {
        return startVideoCall(win)
          .then((response) => {
            cy.wait(500);
            cy.get('#callers-partner-video')
              .then((elements) => {
                const el = elements[0];
                checkVideoDiff(el);
                expect(el.srcObject.getTracks().length).to.be.eq(2);

                response.caller.get_stats_for_peer_connections()
                  .then((stats) => {
                    checkStats(stats);
                  })
              });
            cy.get('#responders-partner-video')
              .then((elements) => {
                const el = elements[0];
                checkVideoDiff(el);
                expect(el.srcObject.getTracks().length).to.be.eq(2);

                response.responder.get_stats_for_peer_connections()
                  .then((stats) => {
                    checkStats(stats);
                  })
              });
          })
      });
  })
});
