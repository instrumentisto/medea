let assert = chai.assert;

function delay(interval)
{
    return it('should delay', done =>
    {
        setTimeout(() => done(), interval)

    }).timeout(interval + 100)
}

describe('Some dummy test', () => {
    after(() => {
        let successEl = document.createElement('div');
        successEl.id = 'test-end';
        document.body.appendChild(successEl);
    })
    delay(2000)
    it('success', () => {
        assert.equal('bar', 'bar');
    })
});
