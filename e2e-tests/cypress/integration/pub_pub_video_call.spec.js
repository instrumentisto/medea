context('Pub<=>Pub video call', () => {
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

  it('open video call and rtc packets sending', () => {
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
            cy.wait(1000);
            cy.get('#callers-partner-video')
              .then((elements) => {
                const el = elements[0];
                checkVideoDiff(el);
                expect(el.srcObject.getTracks().length).to.be.eq(2);

                response.caller.get_stats()
                  .then((stats) => {
                    checkStats(stats);
                  })
              });
            cy.get('#responders-partner-video')
              .then((elements) => {
                const el = elements[0];
                checkVideoDiff(el);
                expect(el.srcObject.getTracks().length).to.be.eq(2);

                response.responder.get_stats()
                  .then((stats) => {
                    checkStats(stats);
                  })
              });
          })
      });
  })
});
