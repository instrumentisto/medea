context('Video call', () => {
  beforeEach(() => {
    cy.request({
      url: 'http://localhost:8000/pub-pub-e2e-call',
      method: 'DELETE',
    });

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

    cy.window()
      .then((win) => {
        return startVideoCall(win)
          .then((response) => {
            cy.wait(1000);
            cy.get('.callers-partner-video')
              .then((el) => {
                expect(el[0].srcObject.getTracks().length).to.be.eq(2);
                response.caller.get_stats()
                  .then((stats) => {
                    checkStats(stats);
                  })
              });
            cy.get('.responders-partner-video')
              .then((el) => {
                response.responder.get_stats()
                  .then((stats) => {
                    expect(el[0].srcObject.getTracks().length).to.be.eq(2);
                    checkStats(stats);
                  })
              });
          })
      });
  })
})
