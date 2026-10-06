// The chart-file loader for webpack and Turbopack (registered by withDatars): a CommonJS shim both
// loader runners can load, around the ES module that does the work.
module.exports = function datarsLoader() {
  const done = this.async();
  const options = typeof this.getOptions === "function" ? this.getOptions() : this.query || {};
  import("./dist/loader.js")
    .then((m) => m.load(this, options))
    .then((code) => done(null, code), (err) => done(err));
};
