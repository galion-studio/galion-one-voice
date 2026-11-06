import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "site"))
from onnxruntime.quantization import quantize_dynamic, QuantType
for n in ["superb_er", "msp_dim"]:
    quantize_dynamic(f"models/{n}.onnx", f"models/{n}.int8.onnx", weight_type=QuantType.QInt8, op_types_to_quantize=["MatMul", "Gemm"])
    print(n, os.path.getsize(f"models/{n}.int8.onnx") // 2**20, "MiB")
