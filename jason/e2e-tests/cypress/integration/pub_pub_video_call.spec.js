context('Video call', () => {
  beforeEach(() => {
    cy.visit('http://localhost:8082')
    cy.request({
      url: 'http://localhost:8000/e2e-test-room',
      method: 'DELETE',
    });
  });

  it('opens', () => {
    function startVideoCall(win) {
      return new Cypress.Promise((resolve, reject) => {
        win.f()
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
            outboundPackets = report.packetsSent;
          } else if (report.type === 'inbound-rtp') {
            inboundPackets = report.packetsReceived;
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
            cy.get('.caller-video')
              .then(() => {
                response.caller.get_stats()
                  .then((stats) => {
                    checkStats(stats);
                  })
              });
            cy.get('.responder-video')
              .then(() => {
                response.responder.get_stats()
                  .then((stats) => {
                    checkStats(stats);
                  })
              });
          })
      });
  })
})