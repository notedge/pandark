import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
export default require("./lib/pandark-darwin-x64.node");
