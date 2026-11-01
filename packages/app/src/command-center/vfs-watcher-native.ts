export async function subscribeVfsWatch(
  path: string,
  ignore?: readonly string[] | string[],
): Promise<string | null> {
  if (typeof window === "undefined" || typeof window.paseoDesktop?.invoke !== "function") {
    return null;
  }

  try {
    const result = await window.paseoDesktop.invoke("watch_directory", {
      path,
      ignore: ignore ? [...ignore] : null,
    });
    if (typeof result === "string" && result.length > 0) {
      return result;
    }
    return null;
  } catch {
    return null;
  }
}

export async function unsubscribeVfsWatch(subscriptionId: string): Promise<boolean> {
  if (typeof window === "undefined" || typeof window.paseoDesktop?.invoke !== "function") {
    return false;
  }

  try {
    const result = await window.paseoDesktop.invoke("unwatch_directory", {
      subscriptionId,
      subscription_id: subscriptionId,
    });
    return result === true;
  } catch {
    return false;
  }
}

export const watchDirectoryNative = subscribeVfsWatch;
export const unwatchDirectoryNative = unsubscribeVfsWatch;
