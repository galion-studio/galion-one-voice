# Experiment (not shipped): ONNX export of the two S3Gen hot parts and ORT-backed
# drop-ins. (1) flow decoder estimator (meanflow U-Net), (2) HiFT conv core
# (conv_pre..conv_post; STFT/iSTFT and the f0/source stay in torch).
# onnxruntime (MIT) is loaded from latency-240/03-pylib, outside the shared venv.
# The .onnx files are derived from the MIT Chatterbox weights; they are not committed.
import os, sys, types, torch, torch.nn.functional as F, numpy as np
sys.path.insert(0, "/workspace/swarm/voice-v2-sprint/latency-240/03-pylib")
import onnxruntime as ort
ODIR = "/workspace/swarm/voice-v2-sprint/latency-240/03-onnx"

class _HiftCore(torch.nn.Module):
    def __init__(self, h): super().__init__(); self.h = h
    def forward(self, x, s_stft):
        h = self.h; x = h.conv_pre(x)
        for i in range(h.num_upsamples):
            x = F.leaky_relu(x, h.lrelu_slope); x = h.ups[i](x)
            if i == h.num_upsamples - 1: x = h.reflection_pad(x)
            si = h.source_resblocks[i](h.source_downs[i](s_stft)); x = x + si
            xs = None
            for j in range(h.num_kernels):
                y = h.resblocks[i * h.num_kernels + j](x); xs = y if xs is None else xs + y
            x = xs / h.num_kernels
        return h.conv_post(F.leaky_relu(x))

def _sess(path, threads):
    so = ort.SessionOptions(); so.intra_op_num_threads = threads; so.inter_op_num_threads = 1
    so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    return ort.InferenceSession(path, so, providers=["CPUExecutionProvider"])

def export(M, force=False):
    os.makedirs(ODIR, exist_ok=True)
    est = M.s3gen.flow.decoder.estimator; T = 300
    p1 = f"{ODIR}/estimator.onnx"
    if force or not os.path.exists(p1):
        args = (torch.randn(1, 80, T), torch.ones(1, 1, T), torch.randn(1, 80, T), torch.tensor([0.0]), torch.randn(1, 80), torch.randn(1, 80, T), torch.tensor([0.5]))
        torch.onnx.export(est, args, p1, input_names=["x", "mask", "mu", "t", "spks", "cond", "r"], output_names=["y"],
            dynamic_axes={k: {2: "T"} for k in ["x", "mask", "mu", "cond", "y"]}, opset_version=17, dynamo=False)
    p2 = f"{ODIR}/hift_core.onnx"
    if force or not os.path.exists(p2):
        h = M.s3gen.mel2wav; core = _HiftCore(h).eval()
        x = torch.randn(1, 80, 100); s = torch.randn(1, 1, 100 * 480)
        sr, si = h._stft(s.squeeze(1)); st = torch.cat([sr, si], 1)
        torch.onnx.export(core, (x, st), p2, input_names=["x", "s_stft"], output_names=["y"],
            dynamic_axes={"x": {2: "T"}, "s_stft": {2: "TS"}, "y": {2: "TO"}}, opset_version=17, dynamo=False)
    return p1, p2

class OrtEstimator(torch.nn.Module):
    def __init__(self, path, threads): super().__init__(); self.s = _sess(path, threads)
    @property
    def dtype(self): return torch.float32
    def forward(self, x, mask, mu, t, spks=None, cond=None, r=None):
        o = self.s.run(None, dict(x=x.numpy(), mask=mask.numpy(), mu=mu.numpy(), t=t.reshape(1).numpy(),
                                  spks=spks.numpy(), cond=cond.numpy(), r=r.reshape(1).numpy()))[0]
        return torch.from_numpy(o)

def use_ort(M, threads=4, parts=("dec", "hift")):
    p1, p2 = export(M)
    if "dec" in parts: M.s3gen.flow.decoder.estimator = OrtEstimator(p1, threads)
    if "hift" in parts:
        h = M.s3gen.mel2wav; sess = _sess(p2, threads)
        def decode(self, x, s=torch.zeros(1, 1, 0)):
            sr, si = self._stft(s.squeeze(1)); st = torch.cat([sr, si], 1)
            y = torch.from_numpy(sess.run(None, dict(x=x.contiguous().numpy(), s_stft=st.contiguous().numpy()))[0])
            nf = self.istft_params["n_fft"] // 2 + 1
            out = self._istft(torch.exp(y[:, :nf, :]), torch.sin(y[:, nf:, :]))
            return torch.clamp(out, -self.audio_limit, self.audio_limit)
        h.decode = types.MethodType(decode, h)
