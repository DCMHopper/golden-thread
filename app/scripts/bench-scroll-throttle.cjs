function throttleRaf(fn) {
  let scheduled = false;
  return (...args) => {
    if (scheduled) return;
    scheduled = true;
    setTimeout(() => {
      scheduled = false;
      fn(...args);
    }, 16);
  };
}

async function run() {
  let directCalls = 0;
  let throttledCalls = 0;

  const direct = () => {
    directCalls += 1;
  };
  const throttled = throttleRaf(() => {
    throttledCalls += 1;
  });

  for (let i = 0; i < 1000; i += 1) {
    direct();
    throttled();
  }

  await new Promise((resolve) => setTimeout(resolve, 25));

  console.log("Direct calls:", directCalls);
  console.log("Throttled calls (single frame):", throttledCalls);
  console.log("Reduction:", `${(directCalls / throttledCalls).toFixed(1)}x`);
}

run();
