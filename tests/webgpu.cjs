// Headless compositors can return a transparent screenshot for a WebGPU canvas.
// Read the presented surface after submission to compare the frames themselves.
module.exports = function observeGpuFrames() {
    let device;
    let surface;
    let sample;
    const request = GPUAdapter.prototype.requestDevice;
    GPUAdapter.prototype.requestDevice = async function (...args) {
        const result = await request.apply(this, args);
        result.addEventListener('uncapturederror', event => window.reportGpuError(event.error.message));
        return result;
    };
    const configure = GPUCanvasContext.prototype.configure;
    GPUCanvasContext.prototype.configure = function (config) {
        device = config.device;
        const usage = (config.usage ?? GPUTextureUsage.RENDER_ATTACHMENT) | GPUTextureUsage.COPY_SRC;
        return configure.call(this, {...config, usage});
    };
    const current = GPUCanvasContext.prototype.getCurrentTexture;
    GPUCanvasContext.prototype.getCurrentTexture = function () {
        surface = current.call(this);
        return surface;
    };
    window.readViewerFrame = () => new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(Error('No WebGPU frame was presented')), 10000);
        sample = promise => {
            clearTimeout(timeout);
            resolve(promise);
        };
    });
    const submit = GPUQueue.prototype.submit;
    GPUQueue.prototype.submit = function (commands) {
        submit.call(this, commands);
        const frame = surface;
        surface = null;
        if (!sample || !frame || this !== device.queue) return;
        const resolve = sample;
        sample = null;
        const {width, height} = frame;
        const bytesPerRow = Math.ceil(width * 4 / 256) * 256;
        const buffer = device.createBuffer({
            size: bytesPerRow * height,
            usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
        });
        const encoder = device.createCommandEncoder();
        encoder.copyTextureToBuffer({texture: frame}, {buffer, bytesPerRow}, {width, height});
        submit.call(this, [encoder.finish()]);
        resolve(buffer.mapAsync(GPUMapMode.READ).then(async () => {
            const hash = await crypto.subtle.digest('SHA-256', buffer.getMappedRange());
            buffer.unmap();
            buffer.destroy();
            return Array.from(new Uint8Array(hash));
        }));
    };
};
