const path = require("path");

const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const HtmlWebpackPlugin = require("html-webpack-plugin");

const dist = path.resolve(__dirname, "dist");

module.exports = {
  mode: 'development',
  entry: "./js/index.js",
  output: {
    path: dist,
    filename: "bundle.js"
  },
  devServer: {
    contentBase: dist
  },
  plugins: [
    new HtmlWebpackPlugin({
      template: 'index.html'
    }),
    new WasmPackPlugin({
      crateDirectory: path.resolve(__dirname, '../'),
      forceMode: 'development'
    })
  ]
};
