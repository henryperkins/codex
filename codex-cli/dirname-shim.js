import { fileURLToPath } from "url";
import { dirname } from "path";

// Define __dirname and __filename for ESM modules
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Make them available globally
globalThis.__dirname = __dirname;
globalThis.__filename = __filename;
