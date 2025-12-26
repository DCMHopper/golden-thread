class FakeContainer {
  constructor() {
    this.appendCount = 0;
    this.replaceCount = 0;
    this.clearCount = 0;
  }
  appendChild() {
    this.appendCount += 1;
  }
  replaceChildren() {
    this.replaceCount += 1;
  }
  set innerHTML(_value) {
    this.clearCount += 1;
  }
}

function renderTagsOld(container, tagCount) {
  container.innerHTML = "";
  for (let i = 0; i < tagCount; i += 1) {
    container.appendChild({});
  }
}

function renderTagsNew(container, tagCount) {
  const fragment = [];
  for (let i = 0; i < tagCount; i += 1) {
    fragment.push({});
  }
  container.replaceChildren(fragment);
}

function run() {
  const tagCount = 12;
  const oldContainer = new FakeContainer();
  renderTagsOld(oldContainer, tagCount);

  const newContainer = new FakeContainer();
  renderTagsNew(newContainer, tagCount);

  console.log(`Tags per message: ${tagCount}`);
  console.log("Old DOM ops:", {
    clears: oldContainer.clearCount,
    appends: oldContainer.appendCount,
    replaces: oldContainer.replaceCount,
    total: oldContainer.clearCount + oldContainer.appendCount + oldContainer.replaceCount,
  });
  console.log("New DOM ops:", {
    clears: newContainer.clearCount,
    appends: newContainer.appendCount,
    replaces: newContainer.replaceCount,
    total: newContainer.clearCount + newContainer.appendCount + newContainer.replaceCount,
  });
}

run();
