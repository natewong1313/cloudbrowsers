export type BrowserContainerRouterId = string & {
  __brand: "BrowserContainerRouterId";
};
export const newBrowserContainerRouterId = (): BrowserContainerRouterId => {
  return crypto.randomUUID() as BrowserContainerRouterId;
};

export type BrowserContainerId = string & { __brand: "BrowserContainerId" };
export const newBrowserContainerId = (): BrowserContainerId => {
  return crypto.randomUUID() as BrowserContainerId;
};
