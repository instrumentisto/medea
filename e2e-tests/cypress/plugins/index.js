// ***********************************************************
// This example plugins/index.js can be used to load plugins
//
// You can change the location of this file or turn off loading
// the plugins file with the 'pluginsFile' configuration option.
//
// You can read more here:
// https://on.cypress.io/plugins-guide
// ***********************************************************

// This function is called when a project is opened or re-opened (e.g. due to
// the project's config changing)

module.exports = (on, config) => {
  on('before:browser:launch', (browser = {}, args) => {
    // You can generate empty video for tests with command
    // "ffmpeg -t 100 -s 640x480 -f rawvideo -pix_fmt rgb24 -r 25 -i /dev/zero empty.mjpeg"
    // then specify path in line below:
    // args.push('--use-file-for-fake-video-capture=/home/relmay/Projects/work/medea/empty.mjpeg');
    return args
  })
};
