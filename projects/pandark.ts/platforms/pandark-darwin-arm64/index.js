import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
export default require("./lib/pandark-darwin-arm64.node");
