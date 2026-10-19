"""Diagnostic fixed-shape MLP only: not a trained music policy or quality result."""
import argparse, json, time, platform
from pathlib import Path
import numpy as np
import openvino as ov
from openvino import opset13 as ops

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--report",default="reports/openvino-probe.json")
    args=parser.parse_args()
    core=ov.Core()
    rng=np.random.default_rng(255)
    x=ops.parameter([1,16],np.float32,name="features")
    w1=rng.normal(0,0.1,(16,32)).astype(np.float32)
    w2=rng.normal(0,0.1,(32,8)).astype(np.float32)
    h=ops.relu(ops.matmul(x,ops.constant(w1),False,False))
    y=ops.tanh(ops.matmul(h,ops.constant(w2),False,False))
    model=ov.Model([y],[x],"diagnostic_mlp_untrained")
    inputs=rng.uniform(-1,1,(12,1,16)).astype(np.float32)
    report={"runtime":ov.__version__,"python":platform.python_version(),"graph":"untrained deterministic 16-32-8 diagnostic MLP; not a music policy","available_devices":core.available_devices,"devices":{},"power":"not measured"}
    outputs={}
    for device in ["CPU","NPU"]:
        item={"enumerated":device in core.available_devices,"compiled":False,"inferred":False}
        report["devices"][device]=item
        try:
            item["full_name"]=core.get_property(device,"FULL_DEVICE_NAME")
            start=time.perf_counter()
            compiled=core.compile_model(model,device,{"PERFORMANCE_HINT":"LATENCY"})
            item["compile_seconds"]=time.perf_counter()-start
            item["compiled"]=True
            item["execution_devices"]=compiled.get_property("EXECUTION_DEVICES")
            request=compiled.create_infer_request()
            for _ in range(5):request.infer({0:inputs[0]})
            results=[]
            for value in inputs:results.append(request.infer({0:value})[compiled.output(0)].copy())
            results=np.array(results)
            item["inferred"]=bool(np.isfinite(results).all())
            item["output_shape"]=list(results.shape)
            times=[]
            for i in range(100):
                start=time.perf_counter_ns()
                request.infer({0:inputs[i%len(inputs)]})
                times.append((time.perf_counter_ns()-start)/1e6)
            item["sync_latency_ms"]={"median":float(np.median(times)),"p95":float(np.percentile(times,95)),"samples":len(times)}
            outputs[device]=results
        except Exception as exc:item["error"]=str(exc)
    if set(outputs)=={"CPU","NPU"}:report["max_absolute_cpu_npu_error"]=float(np.max(np.abs(outputs["CPU"]-outputs["NPU"])))
    Path(args.report).parent.mkdir(parents=True,exist_ok=True)
    Path(args.report).write_text(json.dumps(report,indent=2,ensure_ascii=False),encoding="utf-8")
    print(json.dumps(report,indent=2,ensure_ascii=False))
if __name__=="__main__":main()
