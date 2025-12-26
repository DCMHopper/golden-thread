const { performance } = require("node:perf_hooks");

function buildMessages(count) {
  const list = [];
  for (let i = 0; i < count; i += 1) {
    list.push({ id: `m${i + 1}`, body: `Body ${i + 1}` });
  }
  return list;
}

function benchFind(messages, lookups) {
  const start = performance.now();
  for (let i = 0; i < lookups.length; i += 1) {
    const id = lookups[i];
    const found = messages.find((m) => m.id === id);
    if (!found) {
      throw new Error("missing");
    }
  }
  return performance.now() - start;
}

function benchMap(messages, lookups) {
  const map = new Map(messages.map((m) => [m.id, m]));
  const start = performance.now();
  for (let i = 0; i < lookups.length; i += 1) {
    const id = lookups[i];
    const found = map.get(id);
    if (!found) {
      throw new Error("missing");
    }
  }
  return performance.now() - start;
}

function run() {
  const messages = buildMessages(50_000);
  const lookups = [];
  for (let i = 0; i < 10_000; i += 1) {
    const idx = (i * 37) % messages.length;
    lookups.push(`m${idx + 1}`);
  }

  const findMs = benchFind(messages, lookups);
  const mapMs = benchMap(messages, lookups);

  console.log(`find(): ${findMs.toFixed(2)} ms`);
  console.log(`Map.get(): ${mapMs.toFixed(2)} ms`);
  console.log(`Speedup: ${(findMs / mapMs).toFixed(1)}x`);
}

run();
