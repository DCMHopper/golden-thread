const { performance } = require("node:perf_hooks");

function buildIds(count, prefix = "m") {
  const list = [];
  for (let i = 0; i < count; i += 1) {
    list.push(`${prefix}${i + 1}`);
  }
  return list;
}

function run() {
  const total = 50_000;
  const page = 100;
  const allIds = buildIds(total);
  const newIds = buildIds(page, "m_new_");

  const reactionMap = new Map();
  for (let i = 0; i < total; i += 1) {
    reactionMap.set(allIds[i], []);
  }

  const startFilter = performance.now();
  const missing = allIds.filter((id) => !reactionMap.has(id));
  const filterMs = performance.now() - startFilter;

  const startFilterNew = performance.now();
  const missingNew = newIds.filter((id) => !reactionMap.has(id));
  const filterNewMs = performance.now() - startFilterNew;

  console.log(`Total IDs: ${total}`);
  console.log(`Page IDs: ${page}`);
  console.log(`Old payload size (all IDs): ${allIds.length}`);
  console.log(`New payload size (missing only): ${missingNew.length}`);
  console.log(`Filter time (all IDs): ${filterMs.toFixed(2)} ms`);
  console.log(`Filter time (page IDs): ${filterNewMs.toFixed(2)} ms`);
}

run();
