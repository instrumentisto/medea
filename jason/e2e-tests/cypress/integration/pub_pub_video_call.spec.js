context('Video call', () => {
  beforeEach(() => {
    cy.visit('http://localhost:8082')
    cy.request({
      url: 'http://localhost:8000/e2e-test-room',
      method: 'DELETE',
    });
  })

  it('opens', () => {
    cy.window()
      .then((win) => {
        win.f()
          .then((resp) => {
            setTimeout(() => {
              resp.responder.get_stats()
                .then((resp) => {
                  let outboundPackets = 0;
                  let inboundPackets = 0;
                  resp.forEach(resp => {
                    resp.forEach(report => {
                      if(report.type === 'outbound-rtp') {
                        outboundPackets = report.packetsSent;
                      } else if(report.type === 'inbound-rtp') {
                        inboundPackets = report.packetsReceived;
                      }
                    });
                  });
                  expect(outboundPackets).to.be.greaterThan(5);
                  expect(inboundPackets).to.be.greaterThan(5);
                  cy.continue()
                });
            }, 2000)
          })
      });
    cy.wait(2500);
    // cy.window()
    //   .then((win) => {
    //     console.log(win.medeaEvents.toString());
    //   })
    // cy.window()
    //   .then((win) => {
    //     win.printStats();
    //   });
    // window.startVideoCallSession()
    //   .then(() => {
    //   });
    // cy.windoww()
    //   .then((win) => {
    //     win.startVideoCallSession()
    //       .then((win) => {
    //       })
    //   })
    //   .then((win) => {
    //
    //     console.log("sadf" + win.toString());
    //   })
  })
})
