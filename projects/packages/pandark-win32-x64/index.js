import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
export default require("./lib/pandark-win32-x64-msvc.node");
