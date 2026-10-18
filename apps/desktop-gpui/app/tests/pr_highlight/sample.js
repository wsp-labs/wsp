const path = require("node:path");

function join(a, b) {
  return path.join(a, b);
}

export const TOTAL = 3;
let items = [1, 2].map((n) => n * 2);
module.exports = { join, items };
