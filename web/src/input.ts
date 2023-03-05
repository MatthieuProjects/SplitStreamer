export async function startCapture(displayMediaOptions: DisplayMediaStreamOptions) {
    try {
        return await navigator.mediaDevices
            .getDisplayMedia(displayMediaOptions);
    } catch (err) {
        console.error(`Error:${err}`);
        return null;
    }
}