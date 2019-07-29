/**
 * Promise for wait for element with provided ID to appear.
 * This promise try fro getElementById and if element is not null
 * then promise resolves with that ID.
 */
function waitForElement(id) {
    return new Promise(resolve => {
        let interval = setInterval(() => {
            let waitedEl = document.getElementById(id);
            if(waitedEl != null) {
                clearInterval(interval);
                resolve(waitedEl);
            }
        }, 50)
    })
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
 * Test will fail if difference between this two images are less than 10.
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

    assert.isAtLeast(dataDiff, 10, 'Video which we receiving from partner looks static.');
}

/**
 *  Promise for wait for video to appear.
 *  This promise will check videoWidth parameter
 *  of provided video element. If videoWidth > 0 then
 *  we think that video is loaded.
 */
const waitForVideo = (videoEl) => {
    return new Promise(resolve => {
        let interval = setInterval(() => {
            if(videoEl.videoWidth !== 0) {
                clearInterval(interval);
                resolve()
            }
        }, 50)
    })
};
