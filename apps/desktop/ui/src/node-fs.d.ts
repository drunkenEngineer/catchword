// The one Node function the contrast test uses to read the stylesheet,
// declared here so the interface needs no @types/node package. Tests run
// in Node; the app itself never imports it.
declare module "node:fs" {
  export function readFileSync(path: URL, encoding: "utf8"): string;
}
