(function() {
    const B1 = 1, B8 = 8;
    class RefPointer {
        constructor(id){
            this.id = id;
        }
        get value() {
            return this.id;
        }
    }
    class ExternalPointer extends RefPointer {
    }
    class InternalPointer extends RefPointer {
    }
    class CachePointer extends RefPointer {
    }
    class ErrorCodeValue {
        constructor(code){
            this.code = code;
        }
    }
    class TypedArraySliceValue {
        constructor(sliceType, content){
            this.sliceType = sliceType;
            this.content = content;
        }
    }
    class ReplyContainer {
        constructor(type, value){
            this.type = type;
            this.value = value;
        }
    }
    class FakeNode {
        constructor(tag){
            this.tag = tag;
        }
    }
    class ReplyError extends Error {
        constructor(code, options){
            if (!Number.isInteger(code)) {
                throw new Error("Only numbers allowed to represent the code to be sent");
            }
            super(`Reply failed with error code: ${code}`, options);
            this.code = code;
        }
    }
    const ParamType = Object.freeze({
        Null: 0,
        Undefined: 1,
        Bool: 2,
        Text8: 3,
        Text16: 4,
        Int8: 5,
        Int16: 6,
        Int32: 7,
        Int64: 8,
        Uint8: 9,
        Uint16: 10,
        Uint32: 11,
        Uint64: 12,
        Float32: 13,
        Float64: 14,
        ExternalReference: 15,
        Uint8Array: 16,
        Uint16Array: 17,
        Uint32Array: 18,
        Uint64Array: 19,
        Int8Array: 20,
        Int16Array: 21,
        Int32Array: 22,
        Int64Array: 23,
        Float32Array: 24,
        Float64Array: 25,
        InternalReference: 26,
        Int128: 27,
        Uint128: 28,
        CachedText: 29,
        TypedArraySlice: 30,
        ErrorCode: 31
    });
    const ReturnType = Object.freeze({
        Bool: 1,
        Text8: 2,
        Int8: 3,
        Int16: 4,
        Int32: 5,
        Int64: 6,
        Uint8: 7,
        Uint16: 8,
        Uint32: 9,
        Uint64: 10,
        Float32: 11,
        Float64: 12,
        Int128: 13,
        Uint128: 14,
        MemorySlice: 15,
        ExternalReference: 16,
        InternalReference: 17,
        Object: 28,
        DOMObject: 29,
        None: 30,
        ErrorCode: 31,
        TypedArraySlice: 32
    });
    const RETURN_NAKED = new Set([
        ReturnType.Bool,
        ReturnType.Uint8,
        ReturnType.Uint16,
        ReturnType.Uint32,
        ReturnType.Uint64,
        ReturnType.Int8,
        ReturnType.Int16,
        ReturnType.Int32,
        ReturnType.Int64,
        ReturnType.Float32,
        ReturnType.Float64,
        ReturnType.Object,
        ReturnType.DOMObject,
        ReturnType.ErrorCode,
        ReturnType.MemorySlice,
        ReturnType.InternalReference,
        ReturnType.ExternalReference
    ]);
    const ReturnIds = Object.freeze({
        None: 0,
        One: 1,
        Multi: 2,
        List: 3
    });
    const ThreeStateId = Object.freeze({
        One: 70,
        Two: 80,
        Three: 90
    });
    const ReturnHintMarker = Object.freeze({
        Start: 200,
        Stop: 201
    });
    const ReturnValueMarker = Object.freeze({
        Begin: 100,
        End: 101
    });
    const TypedSliceArray = {
        1: Int8Array,
        2: Int16Array,
        3: Int32Array,
        4: BigInt64Array,
        5: Uint8Array,
        6: Uint16Array,
        7: Uint32Array,
        8: BigUint64Array,
        9: Float32Array,
        10: Float64Array
    };
    class ExternalHeap {
        constructor(){
            this.items = [];
            this.free = [];
        }
        #unpack(uid) {
            const v = BigInt(uid);
            return {
                index: Number(v >> 32n),
                generation: v & 0xffffffffn
            };
        }
        create(item) {
            let index;
            if (this.free.length > 0) {
                index = this.free.pop();
                const slot = this.items[index];
                slot.generation += 1n;
                slot.active = true;
                slot.item = item;
            } else {
                index = this.items.length;
                this.items.push({
                    item,
                    generation: 0n,
                    active: true
                });
            }
            return BigInt(index) << 32n | this.items[index].generation;
        }
        get(uid) {
            const { index, generation } = this.#unpack(uid);
            const slot = this.items[index];
            if (!slot || !slot.active || slot.generation !== generation) return undefined;
            return slot.item;
        }
        update(uid, item) {
            const { index, generation } = this.#unpack(uid);
            const slot = this.items[index];
            if (!slot || slot.generation !== generation) return false;
            slot.item = item;
            slot.active = true;
            return true;
        }
        destroy(uid) {
            const { index, generation } = this.#unpack(uid);
            const slot = this.items[index];
            if (!slot || !slot.active || slot.generation !== generation) return false;
            slot.item = null;
            slot.active = false;
            this.free.push(index);
            return true;
        }
    }
    function asTypedSlice(value) {
        if (value instanceof TypedArraySliceValue) {
            const content = value.content;
            return {
                sliceType: value.sliceType,
                u8: new Uint8Array(content.buffer, content.byteOffset, content.byteLength)
            };
        }
        for (const [id, Ctor] of Object.entries(TypedSliceArray)){
            if (value instanceof Ctor && value.constructor === Ctor) {
                return {
                    sliceType: Number(id),
                    u8: new Uint8Array(value.buffer, value.byteOffset, value.byteLength)
                };
            }
        }
        throw new Error("TypedArraySlice return: expected a TypedArray or TypedArraySliceValue");
    }
    function errorCodeOf(value) {
        if (value instanceof ErrorCodeValue) return value.code;
        if (value && Number.isInteger(value.code)) return value.code;
        if (Number.isInteger(value)) return value;
        if (typeof value === "bigint") return Number(value);
        return 1;
    }
    function matchesReturnType(value, type) {
        switch(type){
            case ReturnType.None:
                return value === undefined || value === null;
            case ReturnType.Bool:
                return typeof value === "boolean";
            case ReturnType.Text8:
                return typeof value === "string";
            case ReturnType.Int8:
            case ReturnType.Int16:
            case ReturnType.Int32:
            case ReturnType.Uint8:
            case ReturnType.Uint16:
            case ReturnType.Uint32:
                return typeof value === "number" && Number.isInteger(value);
            case ReturnType.Int64:
            case ReturnType.Uint64:
            case ReturnType.Int128:
            case ReturnType.Uint128:
                return typeof value === "bigint" || typeof value === "number" && Number.isInteger(value);
            case ReturnType.Float32:
            case ReturnType.Float64:
                return typeof value === "number";
            case ReturnType.ErrorCode:
                return value instanceof ErrorCodeValue || value instanceof ReplyError || typeof value === "number" && Number.isInteger(value);
            case ReturnType.ExternalReference:
                return value instanceof ExternalPointer;
            case ReturnType.InternalReference:
                return value instanceof InternalPointer;
            case ReturnType.TypedArraySlice:
                return value instanceof TypedArraySliceValue || ArrayBuffer.isView(value);
            case ReturnType.MemorySlice:
                return typeof value === "bigint" || typeof value === "number" || ArrayBuffer.isView(value);
            case ReturnType.DOMObject:
                return value instanceof FakeNode || typeof Node !== "undefined" && value instanceof Node || typeof value === "object" && value !== null;
            case ReturnType.Object:
                return typeof value === "object" && value !== null;
            default:
                return false;
        }
    }
    class ParameterParser {
        constructor(bridge, strings){
            this.bridge = bridge;
            this.strings = strings;
        }
        parse(ptr, len) {
            const start = Number(ptr);
            const total = Number(len);
            const view = new DataView(this.bridge.memory.buffer, start, total);
            const args = [];
            let i = 0;
            while(i < total){
                const type = view.getUint8(i);
                i += B1;
                i = this.#one(type, i, view, args);
            }
            return args;
        }
        #slice(view, i) {
            const ptr = Number(view.getBigUint64(i, true));
            i += B8;
            const len = Number(view.getBigUint64(i, true));
            i += B8;
            return [
                i,
                ptr,
                len
            ];
        }
        #one(type, i, view, args) {
            const mem = this.bridge.memory.buffer;
            switch(type){
                case ParamType.Null:
                    args.push(null);
                    return i;
                case ParamType.Undefined:
                    args.push(undefined);
                    return i;
                case ParamType.Bool:
                    args.push(view.getUint8(i) === 1);
                    return i + 1;
                case ParamType.Int8:
                    args.push(view.getInt8(i));
                    return i + 1;
                case ParamType.Uint8:
                    args.push(view.getUint8(i));
                    return i + 1;
                case ParamType.Int16:
                    args.push(view.getInt16(i, true));
                    return i + 2;
                case ParamType.Uint16:
                    args.push(view.getUint16(i, true));
                    return i + 2;
                case ParamType.ErrorCode:
                    args.push(new ErrorCodeValue(view.getUint16(i, true)));
                    return i + 2;
                case ParamType.Int32:
                    args.push(view.getInt32(i, true));
                    return i + 4;
                case ParamType.Uint32:
                    args.push(view.getUint32(i, true));
                    return i + 4;
                case ParamType.Float32:
                    args.push(view.getFloat32(i, true));
                    return i + 4;
                case ParamType.Float64:
                    args.push(view.getFloat64(i, true));
                    return i + 8;
                case ParamType.Int64:
                    args.push(view.getBigInt64(i, true));
                    return i + 8;
                case ParamType.Uint64:
                    args.push(view.getBigUint64(i, true));
                    return i + 8;
                case ParamType.ExternalReference:
                    args.push(new ExternalPointer(view.getBigUint64(i, true)));
                    return i + 8;
                case ParamType.InternalReference:
                    args.push(new InternalPointer(view.getBigUint64(i, true)));
                    return i + 8;
                case ParamType.CachedText:
                    {
                        const handle = view.getBigUint64(i, true);
                        const text = this.strings.get(handle);
                        if (text === undefined) throw new Error(`CachedText: no string for handle ${handle}`);
                        args.push(text);
                        return i + 8;
                    }
                case ParamType.Int128:
                    {
                        const msb = view.getBigInt64(i, true), lsb = view.getBigUint64(i + 8, true);
                        args.push(msb << 64n | lsb);
                        return i + 8 + 8;
                    }
                case ParamType.Uint128:
                    {
                        const msb = view.getBigUint64(i, true), lsb = view.getBigUint64(i + 8, true);
                        args.push(msb << 64n | lsb);
                        return i + 8 + 8;
                    }
                case ParamType.Text8:
                    {
                        const [ni, p, l] = this.#slice(view, i);
                        args.push(new TextDecoder().decode(new Uint8Array(mem, p, l)));
                        return ni;
                    }
                case ParamType.Text16:
                    {
                        const [ni, p, l] = this.#slice(view, i);
                        args.push(new TextDecoder("utf-16le").decode(new Uint8Array(mem, p, l)));
                        return ni;
                    }
                case ParamType.TypedArraySlice:
                    {
                        const sliceType = view.getUint8(i);
                        const [ni, p, l] = this.#slice(view, i + 1);
                        const Ctor = TypedSliceArray[sliceType] ?? Uint8Array;
                        args.push(new TypedArraySliceValue(sliceType, new Ctor(mem.slice(p, p + l * Ctor.BYTES_PER_ELEMENT))));
                        return ni;
                    }
                default:
                    {
                        const Ctor = ARRAY_BUFFER_CTORS[type];
                        if (!Ctor) throw new Error(`ParameterParser: unknown ParamType ${type}`);
                        const [ni, p, l] = this.#slice(view, i);
                        args.push(new Ctor(mem.slice(p, p + l * Ctor.BYTES_PER_ELEMENT)));
                        return ni;
                    }
            }
        }
    }
    const ARRAY_BUFFER_CTORS = {
        [ParamType.Uint8Array]: Uint8Array,
        [ParamType.Uint16Array]: Uint16Array,
        [ParamType.Uint32Array]: Uint32Array,
        [ParamType.Uint64Array]: BigUint64Array,
        [ParamType.Int8Array]: Int8Array,
        [ParamType.Int16Array]: Int16Array,
        [ParamType.Int32Array]: Int32Array,
        [ParamType.Int64Array]: BigInt64Array,
        [ParamType.Float32Array]: Float32Array,
        [ParamType.Float64Array]: Float64Array
    };
    class ReturnHintParser {
        constructor(bridge){
            this.bridge = bridge;
        }
        parse(ptr, len) {
            const view = new DataView(this.bridge.memory.buffer, Number(ptr), Number(len));
            const [, hint] = ReturnHintParser.parseFrom(view, 0);
            return hint;
        }
        static parseFrom(view, i) {
            if (view.getUint8(i) !== ReturnHintMarker.Start) throw new Error("hint: missing Start");
            i += B1;
            const id = view.getUint8(i);
            i += B1;
            const states = [];
            if (id !== ReturnIds.None) {
                while(view.getUint8(i) !== ReturnHintMarker.Stop){
                    const stateId = view.getUint8(i);
                    i += B1;
                    const n = stateId === ThreeStateId.One ? 1 : stateId === ThreeStateId.Two ? 2 : 3;
                    const types = [];
                    for(let k = 0; k < n; k++){
                        types.push(view.getUint8(i));
                        i += B1;
                    }
                    states.push({
                        stateId,
                        types
                    });
                }
            }
            if (view.getUint8(i) !== ReturnHintMarker.Stop) throw new Error("hint: missing Stop");
            i += B1;
            return [
                i,
                {
                    id,
                    states
                }
            ];
        }
    }
    class ReplyEncoder {
        constructor(memory){
            this.memory = memory;
            this.objects = null;
            this.dom = null;
        }
        immediate(hint, value, alwaysEncoded) {
            if (hint.id === ReturnIds.None) {
                if (value === undefined || value === null) return -1n;
                throw new Error(`Expected NoReturn but got ${value}`);
            }
            const containers = this.#containers(hint, value);
            if (hint.id === ReturnIds.One && containers.length === 1 && RETURN_NAKED.has(containers[0].type) && !alwaysEncoded) {
                return this.#naked(containers[0]);
            }
            return this.encodeIntoMemory(containers);
        }
        #naked({ type, value }) {
            switch(type){
                case ReturnType.Object:
                    {
                        if (!this.objects) throw new Error("ReplyEncoder: no object heap wired");
                        return value instanceof RefPointer ? value.value : this.objects.create(value);
                    }
                case ReturnType.DOMObject:
                    {
                        if (!this.dom) throw new Error("ReplyEncoder: no DOM heap wired");
                        return value instanceof RefPointer ? value.value : this.dom.create(value);
                    }
                case ReturnType.ExternalReference:
                case ReturnType.InternalReference:
                    return value instanceof RefPointer ? value.value : value;
                case ReturnType.ErrorCode:
                    return errorCodeOf(value);
                default:
                    return value;
            }
        }
        #containers(hint, value) {
            const resolve = (state, v)=>v instanceof ReplyContainer ? {
                    type: v.type,
                    value: v.value
                } : {
                    type: this.#pickType(state, v),
                    value: v
                };
            switch(hint.id){
                case ReturnIds.One:
                    return [
                        resolve(hint.states[0], value)
                    ];
                case ReturnIds.List:
                    {
                        const state = hint.states[0];
                        return Array.from(value).map((v)=>resolve(state, v));
                    }
                case ReturnIds.Multi:
                    {
                        const arr = Array.from(value);
                        return hint.states.map((state, k)=>resolve(state, arr[k]));
                    }
                default:
                    return [];
            }
        }
        #pickType(state, value) {
            const { types } = state;
            if (types.length === 1) return types[0];
            for (const candidate of types){
                if (matchesReturnType(value, candidate)) return candidate;
            }
            throw new Error(`return value ${value} matches none of the hinted types [${types}]`);
        }
        encodeIntoMemory(containers) {
            const bytes = this.encode(containers);
            const id = this.memory.create(bytes.length);
            this.memory.write(id, bytes);
            return id;
        }
        callbackSuccess(callbacks, callbackId, hint, value) {
            const bytes = this.encode(this.#containers(hint, value));
            callbacks.invoke(callbackId, bytes);
        }
        callbackFailure(callbacks, callbackId, error) {
            const bytes = this.encode([
                {
                    type: ReturnType.ErrorCode,
                    value: errorCodeOf(error)
                }
            ]);
            callbacks.invoke(callbackId, bytes);
        }
        encode(containers) {
            const out = [
                ReturnValueMarker.Begin
            ];
            const push = (n, bytes)=>{
                for(let k = 0; k < bytes; k++)out.push(Number(BigInt(n) >> BigInt(8 * k) & 0xffn));
            };
            for (const { type, value } of containers){
                out.push(type);
                switch(type){
                    case ReturnType.None:
                        break;
                    case ReturnType.Bool:
                        out.push(value ? 1 : 0);
                        break;
                    case ReturnType.Uint8:
                    case ReturnType.Int8:
                        push(value, 1);
                        break;
                    case ReturnType.Uint16:
                    case ReturnType.Int16:
                        push(value, 2);
                        break;
                    case ReturnType.ErrorCode:
                        push(errorCodeOf(value), 2);
                        break;
                    case ReturnType.Uint32:
                    case ReturnType.Int32:
                        push(value, 4);
                        break;
                    case ReturnType.Uint64:
                    case ReturnType.Int64:
                        push(BigInt(value), 8);
                        break;
                    case ReturnType.Float32:
                        {
                            const b = new Uint8Array(4);
                            new DataView(b.buffer).setFloat32(0, value, true);
                            out.push(...b);
                            break;
                        }
                    case ReturnType.Float64:
                        {
                            const b = new Uint8Array(8);
                            new DataView(b.buffer).setFloat64(0, value, true);
                            out.push(...b);
                            break;
                        }
                    case ReturnType.Int128:
                    case ReturnType.Uint128:
                        {
                            const v = BigInt(value);
                            push(v >> 64n & 0xffffffffffffffffn, 8);
                            push(v & 0xffffffffffffffffn, 8);
                            break;
                        }
                    case ReturnType.ExternalReference:
                    case ReturnType.InternalReference:
                        push(BigInt(value instanceof RefPointer ? value.value : value), 8);
                        break;
                    case ReturnType.Text8:
                        {
                            const bytes = new TextEncoder().encode(String(value));
                            const slot = this.memory.create(bytes.length);
                            this.memory.write(slot, bytes);
                            push(slot, 8);
                            break;
                        }
                    case ReturnType.MemorySlice:
                        {
                            if (typeof value === "bigint" || typeof value === "number") {
                                push(BigInt(value), 8);
                                break;
                            }
                            const u8 = value instanceof Uint8Array ? value : new Uint8Array(value.buffer ?? value);
                            const slot = this.memory.create(u8.length);
                            this.memory.write(slot, u8);
                            push(slot, 8);
                            break;
                        }
                    case ReturnType.Object:
                        {
                            if (!this.objects) throw new Error("ReplyEncoder: no object heap wired");
                            push(value instanceof RefPointer ? value.value : this.objects.create(value), 8);
                            break;
                        }
                    case ReturnType.DOMObject:
                        {
                            if (!this.dom) throw new Error("ReplyEncoder: no DOM heap wired");
                            push(value instanceof RefPointer ? value.value : this.dom.create(value), 8);
                            break;
                        }
                    case ReturnType.TypedArraySlice:
                        {
                            const { sliceType, u8 } = asTypedSlice(value);
                            const slot = this.memory.create(u8.length);
                            this.memory.write(slot, u8);
                            out.push(sliceType);
                            push(this.memory.get(slot).ptr, 8);
                            push(u8.length, 8);
                            break;
                        }
                    default:
                        throw new Error(`ReplyEncoder: unsupported return type ${type}`);
                }
            }
            out.push(ReturnValueMarker.End);
            return Uint8Array.from(out);
        }
    }
    class FunctionRegistry {
        constructor(bridge, memory, strings, callbacks){
            this.bridge = bridge;
            this.params = new ParameterParser(bridge, strings);
            this.hints = new ReturnHintParser(bridge);
            this.reply = new ReplyEncoder(memory);
            this.callbacks = callbacks;
            this.heap = new ExternalHeap();
            this.context = this;
        }
        static compile(source) {
            return Function(`"use strict"; return(${source})`)();
        }
        register(start, len, utf) {
            const enc = Number(utf) === 16 ? "utf-16le" : "utf-8";
            const bytes = new Uint8Array(this.bridge.memory.buffer, Number(start), Number(len));
            const source = new TextDecoder(enc).decode(bytes);
            return this.heap.create(FunctionRegistry.compile(source));
        }
        allocate() {
            return this.heap.create(null);
        }
        unregister(handle) {
            this.heap.destroy(BigInt(handle));
        }
        #call(handle, pPtr, pLen) {
            const fn = this.heap.get(BigInt(handle));
            if (!fn) throw new Error(`invoke: no function for handle ${handle}`);
            return fn.apply(this.context, this.params.parse(pPtr, pLen));
        }
        invoke(handle, pPtr, pLen, rPtr, rLen) {
            const hint = this.hints.parse(rPtr, rLen);
            const result = this.#call(handle, pPtr, pLen);
            const reply = this.reply.immediate(hint, result, true);
            return typeof reply === "bigint" ? reply : BigInt(reply);
        }
        invokeNakedAs(handle, pPtr, pLen, returnTypeId) {
            const hint = {
                id: ReturnIds.One,
                states: [
                    {
                        stateId: ThreeStateId.One,
                        types: [
                            returnTypeId
                        ]
                    }
                ]
            };
            return this.reply.immediate(hint, this.#call(handle, pPtr, pLen), false);
        }
        invokeAsync(handle, callbackHandle, pPtr, pLen, rPtr, rLen) {
            const hint = this.hints.parse(rPtr, rLen);
            const result = this.#call(handle, pPtr, pLen);
            if (hint.id === ReturnIds.None) return;
            const callbackId = BigInt(callbackHandle);
            const settled = Promise.resolve(result).then((value)=>this.reply.callbackSuccess(this.callbacks, callbackId, hint, value), (error)=>this.reply.callbackFailure(this.callbacks, callbackId, error));
            if (this.tasks) this.tasks.add(settled);
        }
        invokeAsBool(handle, pPtr, pLen) {
            return this.invokeNakedAs(handle, pPtr, pLen, ReturnType.Bool) ? 1 : 0;
        }
        invokeAsFloat(handle, pPtr, pLen) {
            return Number(this.invokeNakedAs(handle, pPtr, pLen, ReturnType.Float64));
        }
        invokeAsInt(handle, pPtr, pLen) {
            const v = this.invokeNakedAs(handle, pPtr, pLen, ReturnType.Int64);
            return typeof v === "bigint" ? Number(v) : v;
        }
        invokeAsBigInt(handle, pPtr, pLen) {
            const v = this.invokeNakedAs(handle, pPtr, pLen, ReturnType.Uint64);
            return typeof v === "bigint" ? v : BigInt(v);
        }
        invokeAsObject(handle, pPtr, pLen) {
            const v = this.invokeNakedAs(handle, pPtr, pLen, ReturnType.Object);
            return typeof v === "bigint" ? v : BigInt(v);
        }
        invokeAsString(handle, pPtr, pLen) {
            const bytes = new TextEncoder().encode(String(this.#call(handle, pPtr, pLen)));
            const slot = this.reply.memory.create(bytes.length);
            this.reply.memory.write(slot, bytes);
            return slot;
        }
        asNone() {
            return new ReplyContainer(ReturnType.None, undefined);
        }
        asBool(v) {
            return new ReplyContainer(ReturnType.Bool, Boolean(v));
        }
        asText8(v) {
            return new ReplyContainer(ReturnType.Text8, String(v));
        }
        asInt8(v) {
            return new ReplyContainer(ReturnType.Int8, v);
        }
        asInt16(v) {
            return new ReplyContainer(ReturnType.Int16, v);
        }
        asInt32(v) {
            return new ReplyContainer(ReturnType.Int32, v);
        }
        asInt64(v) {
            return new ReplyContainer(ReturnType.Int64, BigInt(v));
        }
        asUint8(v) {
            return new ReplyContainer(ReturnType.Uint8, v);
        }
        asUint16(v) {
            return new ReplyContainer(ReturnType.Uint16, v);
        }
        asUint32(v) {
            return new ReplyContainer(ReturnType.Uint32, v);
        }
        asUint64(v) {
            return new ReplyContainer(ReturnType.Uint64, BigInt(v));
        }
        asFloat32(v) {
            return new ReplyContainer(ReturnType.Float32, v);
        }
        asFloat64(v) {
            return new ReplyContainer(ReturnType.Float64, v);
        }
        asInt128(lsb, msb = 0) {
            return new ReplyContainer(ReturnType.Int128, BigInt(msb) << 64n | BigInt(lsb) & 0xffffffffffffffffn);
        }
        asUint128(lsb, msb = 0) {
            return new ReplyContainer(ReturnType.Uint128, BigInt(msb) << 64n | BigInt(lsb) & 0xffffffffffffffffn);
        }
        asErrorCode(v) {
            return new ReplyContainer(ReturnType.ErrorCode, v instanceof ErrorCodeValue ? v.code : v);
        }
        asReplyError(code) {
            return new ReplyError(code);
        }
        asObject(value) {
            if (typeof value !== "object" || value === null) {
                throw new Error("Value must be a JS Object/object");
            }
            if (!this.reply.objects) throw new Error("asObject: no object heap wired");
            return new ReplyContainer(ReturnType.Object, new ExternalPointer(this.reply.objects.create(value)));
        }
        asDOMObject(value) {
            if (!this.reply.dom) throw new Error("asDOMObject: no DOM heap wired");
            return new ReplyContainer(ReturnType.DOMObject, new InternalPointer(this.reply.dom.create(value)));
        }
        asFakeNode(tag) {
            if (typeof tag !== "string") throw new Error("Value must be a JS string");
            return this.asDOMObject(new FakeNode(tag));
        }
        asMemorySlice(value) {
            const id = typeof value === "bigint" || typeof value === "number" ? BigInt(value) : value;
            return new ReplyContainer(ReturnType.MemorySlice, id);
        }
        asTypedArraySlice(sliceType, content) {
            return new ReplyContainer(ReturnType.TypedArraySlice, new TypedArraySliceValue(sliceType, content));
        }
        asInternalReference(v) {
            return new ReplyContainer(ReturnType.InternalReference, v instanceof RefPointer ? v : new InternalPointer(BigInt(v)));
        }
        asExternalReference(v) {
            return new ReplyContainer(ReturnType.ExternalReference, v instanceof RefPointer ? v : new ExternalPointer(BigInt(v)));
        }
        #asArray(Ctor, v) {
            if (!(v instanceof Ctor)) throw new Error(`Value must be a ${Ctor.name}`);
            return new ReplyContainer(ReturnType.MemorySlice, new Uint8Array(v.buffer, v.byteOffset, v.byteLength));
        }
        asUint8Array(v) {
            return this.#asArray(Uint8Array, v);
        }
        asUint16Array(v) {
            return this.#asArray(Uint16Array, v);
        }
        asUint32Array(v) {
            return this.#asArray(Uint32Array, v);
        }
        asUint64Array(v) {
            return this.#asArray(BigUint64Array, v);
        }
        asInt8Array(v) {
            return this.#asArray(Int8Array, v);
        }
        asInt16Array(v) {
            return this.#asArray(Int16Array, v);
        }
        asInt32Array(v) {
            return this.#asArray(Int32Array, v);
        }
        asInt64Array(v) {
            return this.#asArray(BigInt64Array, v);
        }
        asFloat32Array(v) {
            return this.#asArray(Float32Array, v);
        }
        asFloat64Array(v) {
            return this.#asArray(Float64Array, v);
        }
    }
    const Operations = Object.freeze({
        Begin: 0,
        MakeFunction: 1,
        Invoke: 2,
        InvokeAsync: 3,
        End: 254,
        Stop: 255
    });
    const ArgumentOperations = Object.freeze({
        Start: 1,
        Begin: 2,
        End: 3,
        Stop: 4
    });
    const TypeOptimization = Object.freeze({
        None: 0,
        QuantizedInt16AsI8: 1,
        QuantizedInt32AsI8: 2,
        QuantizedInt32AsI16: 3,
        QuantizedInt64AsI8: 4,
        QuantizedInt64AsI16: 5,
        QuantizedInt64AsI32: 6,
        QuantizedUint16AsU8: 7,
        QuantizedUint32AsU8: 8,
        QuantizedUint32AsU16: 9,
        QuantizedUint64AsU8: 10,
        QuantizedUint64AsU16: 11,
        QuantizedUint64AsU32: 12,
        QuantizedF64AsF32: 13,
        QuantizedF128AsF32: 14,
        QuantizedF128AsF64: 15,
        QuantizedInt128AsI8: 16,
        QuantizedInt128AsI16: 17,
        QuantizedInt128AsI32: 18,
        QuantizedInt128AsI64: 19,
        QuantizedUint128AsU8: 20,
        QuantizedUint128AsU16: 21,
        QuantizedUint128AsU32: 22,
        QuantizedUint128AsU64: 23,
        QuantizedPtrAsU8: 24,
        QuantizedPtrAsU16: 25,
        QuantizedPtrAsU32: 26,
        QuantizedPtrAsU64: 27
    });
    const GroupReturnHintMarker = Object.freeze({
        Start: 111,
        Stop: 222
    });
    function readQuantized(view, i, none) {
        const tq = view.getUint8(i);
        i += B1;
        switch(tq){
            case TypeOptimization.None:
                return none(view, i);
            case TypeOptimization.QuantizedInt16AsI8:
            case TypeOptimization.QuantizedInt32AsI8:
            case TypeOptimization.QuantizedInt64AsI8:
            case TypeOptimization.QuantizedInt128AsI8:
                return [
                    i + 1,
                    view.getInt8(i)
                ];
            case TypeOptimization.QuantizedInt32AsI16:
            case TypeOptimization.QuantizedInt64AsI16:
            case TypeOptimization.QuantizedInt128AsI16:
                return [
                    i + 2,
                    view.getInt16(i, true)
                ];
            case TypeOptimization.QuantizedInt64AsI32:
            case TypeOptimization.QuantizedInt128AsI32:
                return [
                    i + 4,
                    view.getInt32(i, true)
                ];
            case TypeOptimization.QuantizedInt128AsI64:
                return [
                    i + 8,
                    view.getBigInt64(i, true)
                ];
            case TypeOptimization.QuantizedUint16AsU8:
            case TypeOptimization.QuantizedUint32AsU8:
            case TypeOptimization.QuantizedUint64AsU8:
            case TypeOptimization.QuantizedUint128AsU8:
            case TypeOptimization.QuantizedPtrAsU8:
                return [
                    i + 1,
                    view.getUint8(i)
                ];
            case TypeOptimization.QuantizedUint32AsU16:
            case TypeOptimization.QuantizedUint64AsU16:
            case TypeOptimization.QuantizedUint128AsU16:
            case TypeOptimization.QuantizedPtrAsU16:
                return [
                    i + 2,
                    view.getUint16(i, true)
                ];
            case TypeOptimization.QuantizedUint64AsU32:
            case TypeOptimization.QuantizedUint128AsU32:
            case TypeOptimization.QuantizedPtrAsU32:
                return [
                    i + 4,
                    view.getUint32(i, true)
                ];
            case TypeOptimization.QuantizedUint128AsU64:
            case TypeOptimization.QuantizedPtrAsU64:
                return [
                    i + 8,
                    view.getBigUint64(i, true)
                ];
            case TypeOptimization.QuantizedF64AsF32:
            case TypeOptimization.QuantizedF128AsF32:
                return [
                    i + 4,
                    view.getFloat32(i, true)
                ];
            case TypeOptimization.QuantizedF128AsF64:
                return [
                    i + 8,
                    view.getFloat64(i, true)
                ];
            default:
                throw new Error(`readQuantized: unknown TypeOptimization ${tq}`);
        }
    }
    const noneI16 = (v, i)=>[
            i + 2,
            v.getInt16(i, true)
        ];
    const noneU16 = (v, i)=>[
            i + 2,
            v.getUint16(i, true)
        ];
    const noneI32 = (v, i)=>[
            i + 4,
            v.getInt32(i, true)
        ];
    const noneU32 = (v, i)=>[
            i + 4,
            v.getUint32(i, true)
        ];
    const noneI64 = (v, i)=>[
            i + 8,
            v.getBigInt64(i, true)
        ];
    const noneU64 = (v, i)=>[
            i + 8,
            v.getBigUint64(i, true)
        ];
    const noneF64 = (v, i)=>[
            i + 8,
            v.getFloat64(i, true)
        ];
    const noneI128 = (v, i)=>[
            i + 16,
            v.getBigInt64(i, true) << 64n | v.getBigUint64(i + 8, true)
        ];
    const noneU128 = (v, i)=>[
            i + 16,
            v.getBigUint64(i, true) << 64n | v.getBigUint64(i + 8, true)
        ];
    class BatchParameterParser {
        constructor(bridge, strings){
            this.bridge = bridge;
            this.strings = strings;
        }
        parseParams(view, i, texts) {
            const args = [];
            while(view.getUint8(i) !== ArgumentOperations.Stop){
                if (view.getUint8(i) !== ArgumentOperations.Begin) {
                    throw new Error(`batch args: expected Begin marker, got ${view.getUint8(i)}`);
                }
                i += B1;
                let value;
                [i, value] = this.parseParam(view, i, texts);
                args.push(value);
                if (view.getUint8(i) !== ArgumentOperations.End) {
                    throw new Error(`batch args: expected End marker, got ${view.getUint8(i)}`);
                }
                i += B1;
            }
            i += B1;
            return [
                i,
                args
            ];
        }
        parseParam(view, i, texts) {
            const type = view.getUint8(i);
            i += B1;
            switch(type){
                case ParamType.Null:
                    return [
                        i,
                        null
                    ];
                case ParamType.Undefined:
                    return [
                        i,
                        undefined
                    ];
                case ParamType.Bool:
                    return [
                        i + 1,
                        view.getUint8(i) === 1
                    ];
                case ParamType.Int8:
                    return [
                        i + 1,
                        view.getInt8(i)
                    ];
                case ParamType.Uint8:
                    return [
                        i + 1,
                        view.getUint8(i)
                    ];
                case ParamType.Float32:
                    return [
                        i + 4,
                        view.getFloat32(i, true)
                    ];
                case ParamType.Int16:
                    return readQuantized(view, i, noneI16);
                case ParamType.Uint16:
                    return readQuantized(view, i, noneU16);
                case ParamType.Int32:
                    return readQuantized(view, i, noneI32);
                case ParamType.Uint32:
                    return readQuantized(view, i, noneU32);
                case ParamType.Int64:
                    return readQuantized(view, i, noneI64);
                case ParamType.Uint64:
                    return readQuantized(view, i, noneU64);
                case ParamType.Float64:
                    return readQuantized(view, i, noneF64);
                case ParamType.Int128:
                    return readQuantized(view, i, noneI128);
                case ParamType.Uint128:
                    return readQuantized(view, i, noneU128);
                case ParamType.ErrorCode:
                    {
                        const [ni, code] = readQuantized(view, i, noneU16);
                        return [
                            ni,
                            new ErrorCodeValue(Number(code))
                        ];
                    }
                case ParamType.CachedText:
                    {
                        const [ni, handle] = readQuantized(view, i, noneU64);
                        const text = this.strings.get(BigInt(handle));
                        if (text === undefined) throw new Error(`batch CachedText: no string for handle ${handle}`);
                        return [
                            ni,
                            text
                        ];
                    }
                case ParamType.ExternalReference:
                    {
                        const [ni, id] = readQuantized(view, i, noneU64);
                        return [
                            ni,
                            new ExternalPointer(BigInt(id))
                        ];
                    }
                case ParamType.InternalReference:
                    {
                        const [ni, id] = readQuantized(view, i, noneU64);
                        return [
                            ni,
                            new InternalPointer(BigInt(id))
                        ];
                    }
                case ParamType.Text8:
                    {
                        let index, length;
                        [i, index] = readQuantized(view, i, noneU64);
                        [i, length] = readQuantized(view, i, noneU64);
                        return [
                            i,
                            texts.substr(Number(index), Number(length))
                        ];
                    }
                case ParamType.Text16:
                    {
                        let ptr, length;
                        [i, ptr] = readQuantized(view, i, noneU64);
                        [i, length] = readQuantized(view, i, noneU64);
                        const bytes = new Uint8Array(this.bridge.memory.buffer, Number(ptr), Number(length) * 2);
                        return [
                            i,
                            new TextDecoder("utf-16le").decode(bytes)
                        ];
                    }
                case ParamType.TypedArraySlice:
                    {
                        const sliceType = view.getUint8(i);
                        i += B1;
                        let ptr, length;
                        [i, ptr] = readQuantized(view, i, noneU64);
                        [i, length] = readQuantized(view, i, noneU64);
                        const copy = this.bridge.memory.buffer.slice(Number(ptr), Number(ptr) + Number(length));
                        const Ctor = TypedSliceArray[sliceType] ?? Uint8Array;
                        return [
                            i,
                            new TypedArraySliceValue(sliceType, new Ctor(copy))
                        ];
                    }
                default:
                    {
                        const Ctor = BATCH_ARRAY_CTORS[type];
                        if (!Ctor) throw new Error(`batch param: unknown ParamType ${type}`);
                        let ptr, length;
                        [i, ptr] = readQuantized(view, i, noneU64);
                        [i, length] = readQuantized(view, i, noneU64);
                        const start = Number(ptr);
                        const copy = this.bridge.memory.buffer.slice(start, start + Number(length) * Ctor.BYTES_PER_ELEMENT);
                        return [
                            i,
                            new Ctor(copy)
                        ];
                    }
            }
        }
    }
    const BATCH_ARRAY_CTORS = {
        [ParamType.Uint8Array]: Uint8Array,
        [ParamType.Uint16Array]: Uint16Array,
        [ParamType.Uint32Array]: Uint32Array,
        [ParamType.Uint64Array]: BigUint64Array,
        [ParamType.Int8Array]: Int8Array,
        [ParamType.Int16Array]: Int16Array,
        [ParamType.Int32Array]: Int32Array,
        [ParamType.Int64Array]: BigInt64Array,
        [ParamType.Float32Array]: Float32Array,
        [ParamType.Float64Array]: Float64Array
    };
    const MAKE_FUNCTION_HINT = Object.freeze({
        id: ReturnIds.One,
        states: [
            {
                stateId: ThreeStateId.One,
                types: [
                    ReturnType.ExternalReference
                ]
            }
        ]
    });
    class BatchInstructions {
        constructor(bridge, memory, strings, functions, callbacks){
            this.bridge = bridge;
            this.memory = memory;
            this.functions = functions;
            this.callbacks = callbacks;
            this.reply = functions.reply;
            this.params = new BatchParameterParser(bridge, strings);
            this.operations = new Map([
                [
                    Operations.MakeFunction,
                    BatchInstructions.makeFunction
                ],
                [
                    Operations.Invoke,
                    BatchInstructions.invoke
                ],
                [
                    Operations.InvokeAsync,
                    BatchInstructions.invokeAsync
                ]
            ]);
        }
        registerOperation(opId, handler) {
            this.operations.set(opId, handler);
            return this;
        }
        applyNoReturn(opsPtr, opsLen, textPtr, textLen) {
            for (const thunk of this.#parse(opsPtr, opsLen, textPtr, textLen)){
                thunk(this);
            }
        }
        applyReturning(opsPtr, opsLen, textPtr, textLen) {
            const results = [];
            for (const thunk of this.#parse(opsPtr, opsLen, textPtr, textLen)){
                const result = thunk(this);
                if (result === null || result === undefined) continue;
                const memId = this.reply.immediate(result.hint, result.value, true);
                results.push({
                    hint: result.hint,
                    memId: BigInt(memId)
                });
            }
            if (results.length === 0) return -1n;
            const out = [
                GroupReturnHintMarker.Start
            ];
            for (const { hint, memId } of results){
                out.push(hint.id);
                if (hint.id === ReturnIds.Multi) {
                    out.push(Number(hint.states.length & 0xff), Number(hint.states.length >> 8 & 0xff));
                }
                for (const state of hint.states){
                    out.push(state.stateId, ...state.types);
                }
                for(let k = 0; k < 8; k++)out.push(Number(memId >> BigInt(8 * k) & 0xffn));
            }
            out.push(GroupReturnHintMarker.Stop);
            const bytes = Uint8Array.from(out);
            const slot = this.memory.create(bytes.length);
            this.memory.write(slot, bytes);
            return slot;
        }
        #parse(opsPtr, opsLen, textPtr, textLen) {
            const opsStart = Number(opsPtr);
            const ops = this.bridge.memory.buffer.slice(opsStart, opsStart + Number(opsLen));
            const textStart = Number(textPtr);
            const textBytes = new Uint8Array(this.bridge.memory.buffer, textStart, Number(textLen));
            const texts = new TextDecoder().decode(textBytes);
            const view = new DataView(ops);
            let i = 0;
            if (view.getUint8(i) !== Operations.Begin) {
                throw new Error(`batch: expected Operations.Begin, got ${view.getUint8(i)}`);
            }
            i += B1;
            const thunks = [];
            while(i < view.byteLength && view.getUint8(i) !== Operations.Stop){
                const opId = view.getUint8(i);
                i += B1;
                const handler = this.operations.get(opId);
                if (!handler) throw new Error(`batch: unhandled operation ${opId}`);
                let thunk;
                [i, thunk] = handler(this, opId, i, view, texts);
                if (thunk) thunks.push(thunk);
            }
            return thunks;
        }
        static readExternalHandle(view, i) {
            const vt = view.getUint8(i);
            if (vt !== ParamType.ExternalReference) {
                throw new Error(`batch: expected ExternalReference param, got ${vt}`);
            }
            const [ni, raw] = readQuantized(view, i + 1, noneU64);
            return [
                ni,
                BigInt(raw)
            ];
        }
        static makeFunction(batch, _opId, i, view, texts) {
            let handle;
            [i, handle] = BatchInstructions.readExternalHandle(view, i);
            if (view.getUint8(i) !== ParamType.Text8) {
                throw new Error(`batch MakeFunction: expected Text8, got ${view.getUint8(i)}`);
            }
            i += B1;
            const index = Number(view.getBigUint64(i, true));
            i += B8;
            const length = Number(view.getBigUint64(i, true));
            i += B8;
            if (view.getUint8(i) !== Operations.End) {
                throw new Error(`batch MakeFunction: expected Operations.End, got ${view.getUint8(i)}`);
            }
            i += B1;
            const source = texts.substr(index, length);
            const thunk = (b)=>{
                const fn = b.functions.constructor.compile(source);
                b.functions.heap.update(handle, fn);
                return {
                    hint: MAKE_FUNCTION_HINT,
                    value: new ExternalPointer(handle)
                };
            };
            return [
                i,
                thunk
            ];
        }
        static parseCallParts(batch, i, view, texts, hasCallback) {
            let handle;
            [i, handle] = BatchInstructions.readExternalHandle(view, i);
            let callbackId = null;
            if (hasCallback) {
                const vt = view.getUint8(i);
                if (vt !== ParamType.InternalReference) {
                    throw new Error(`batch: expected InternalReference param, got ${vt}`);
                }
                let raw;
                [i, raw] = readQuantized(view, i + B1, noneU64);
                callbackId = BigInt(raw);
            }
            let hint;
            [i, hint] = ReturnHintParser.parseFrom(view, i);
            if (view.getUint8(i) !== ArgumentOperations.Start) {
                throw new Error(`batch args: expected Start marker, got ${view.getUint8(i)}`);
            }
            i += B1;
            let args;
            [i, args] = batch.params.parseParams(view, i, texts);
            if (view.getUint8(i) !== Operations.End) {
                throw new Error(`batch: expected Operations.End, got ${view.getUint8(i)}`);
            }
            i += B1;
            return [
                i,
                handle,
                callbackId,
                hint,
                args
            ];
        }
        static invoke(batch, _opId, i, view, texts) {
            let handle, hint, args;
            [i, handle, , hint, args] = BatchInstructions.parseCallParts(batch, i, view, texts, false);
            const thunk = (b)=>{
                const fn = b.functions.heap.get(handle);
                if (!fn) throw new Error(`batch invoke: no function for handle ${handle}`);
                const result = fn.apply(b.functions.context, args);
                if (hint.id === ReturnIds.None) return null;
                return {
                    hint,
                    value: result
                };
            };
            return [
                i,
                thunk
            ];
        }
        static invokeAsync(batch, _opId, i, view, texts) {
            let handle, callbackId, hint, args;
            [i, handle, callbackId, hint, args] = BatchInstructions.parseCallParts(batch, i, view, texts, true);
            const thunk = (b)=>{
                const fn = b.functions.heap.get(handle);
                if (!fn) throw new Error(`batch invokeAsync: no function for handle ${handle}`);
                const result = fn.apply(b.functions.context, args);
                if (hint.id === ReturnIds.None) return null;
                const settled = Promise.resolve(result).then((value)=>b.reply.callbackSuccess(b.callbacks, callbackId, hint, value), (error)=>b.reply.callbackFailure(b.callbacks, callbackId, error));
                if (b.functions.tasks) b.functions.tasks.add(settled);
                return null;
            };
            return [
                i,
                thunk
            ];
        }
    }
    class WasmEnvelope {
        static HEADER_LEN = 14;
        static write(protocol, version, memoryId, payload) {
            const buf = new Uint8Array(WasmEnvelope.HEADER_LEN + payload.length);
            const view = new DataView(buf.buffer);
            view.setUint8(0, protocol);
            view.setUint8(1, version);
            view.setBigUint64(2, BigInt(memoryId), true);
            view.setUint32(10, payload.length, true);
            buf.set(payload, WasmEnvelope.HEADER_LEN);
            return buf;
        }
        static parse(bytes, byteOffset = 0) {
            const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes, byteOffset);
            if (u8.length < WasmEnvelope.HEADER_LEN) {
                throw new RangeError("WasmEnvelope: buffer shorter than 14-byte header");
            }
            const view = new DataView(u8.buffer, u8.byteOffset, u8.byteLength);
            const protocol = view.getUint8(0);
            const version = view.getUint8(1);
            const memoryId = view.getBigUint64(2, true);
            const length = view.getUint32(10, true);
            const end = WasmEnvelope.HEADER_LEN + length;
            if (u8.length < end) {
                throw new RangeError("WasmEnvelope: payload length exceeds buffer");
            }
            return {
                protocol,
                version,
                memoryId,
                length,
                payload: u8.subarray(WasmEnvelope.HEADER_LEN, end)
            };
        }
    }
    class ProtocolDispatcher {
        constructor(handlers = {}){
            this.handlers = handlers;
        }
        setHandler(protocol, handler) {
            this.handlers[protocol] = handler;
            return this;
        }
        setRawHandler(matches, apply) {
            this.rawHandler = {
                matches,
                apply
            };
        }
        dispatch(buffer) {
            if (this.rawHandler?.matches(buffer)) return this.rawHandler.apply(buffer);
            const { protocol, memoryId, payload } = WasmEnvelope.parse(buffer);
            const handler = this.handlers[protocol];
            if (!handler) {
                throw new Error(`unknown protocol: ${protocol}`);
            }
            return handler.apply(memoryId, payload);
        }
    }
    class MemoryAllocations {
        constructor(bridge){
            this.bridge = bridge;
        }
        create(size) {
            return this.bridge.exports.create_allocation(BigInt(size));
        }
        get(id) {
            const ptr = Number(this.bridge.exports.allocation_start_pointer(BigInt(id)));
            const len = Number(this.bridge.exports.allocation_length(BigInt(id)));
            return {
                ptr,
                len,
                bytes: new Uint8Array(this.bridge.memory.buffer, ptr, len)
            };
        }
        write(id, data) {
            this.get(id).bytes.set(data);
        }
        dispose(id) {
            this.bridge.exports.dispose_allocation(BigInt(id));
        }
        clear(id) {
            this.bridge.exports.clear_allocation(BigInt(id));
        }
    }
    class TimerRegistry {
        constructor(bridge, host = globalThis){
            this.bridge = bridge;
            this.host = host;
            this.timeouts = new Map();
            this.intervals = new Map();
            this.disposed = false;
        }
        scheduleTimeout(callbackId, delayMs) {
            if (this.disposed) return;
            const key = String(callbackId);
            const handle = this.host.setTimeout(()=>{
                this.timeouts.delete(key);
                if (this.disposed) return;
                this.bridge.exports.run_scheduled_callback(BigInt(callbackId));
            }, delayMs);
            this.timeouts.set(key, handle);
        }
        scheduleMicrotask(callbackId) {
            if (this.disposed) return;
            const queue = this.host.queueMicrotask ? this.host.queueMicrotask.bind(this.host) : (fn)=>Promise.resolve().then(fn);
            queue(()=>{
                if (this.disposed) return;
                this.bridge.exports.run_scheduled_callback(BigInt(callbackId));
            });
        }
        cancelTimeout(callbackId) {
            const key = String(callbackId);
            const handle = this.timeouts.get(key);
            if (handle !== undefined) {
                this.host.clearTimeout(handle);
                this.timeouts.delete(key);
            }
        }
        scheduleInterval(callbackId, intervalMs) {
            if (this.disposed) return;
            const key = String(callbackId);
            const handle = this.host.setInterval(()=>{
                if (Number(this.bridge.exports.run_interval_callback(BigInt(callbackId))) === 0) {
                    this.cancelInterval(callbackId);
                }
            }, intervalMs);
            this.intervals.set(key, handle);
        }
        cancelInterval(callbackId) {
            const key = String(callbackId);
            const handle = this.intervals.get(key);
            if (handle !== undefined) {
                this.host.clearInterval(handle);
                this.intervals.delete(key);
            }
        }
        cancelAll() {
            this.disposed = true;
            for (const handle of this.timeouts.values())this.host.clearTimeout(handle);
            for (const handle of this.intervals.values())this.host.clearInterval(handle);
            this.timeouts.clear();
            this.intervals.clear();
        }
    }
    class CallbackRegistry {
        constructor(bridge, memory){
            this.bridge = bridge;
            this.memory = memory;
        }
        invoke(id, data) {
            const slot = this.memory.create(data.length);
            this.memory.write(slot, data);
            this.bridge.exports.invoke_callback(BigInt(id), BigInt(slot));
        }
        unregister(id) {
            this.bridge.exports.unregister_callback(BigInt(id));
        }
    }
    class StringCache {
        constructor(bridge){
            this.bridge = bridge;
            this.byString = new Map();
            this.byHandle = new Map();
            this.next = 1n;
        }
        cache(ptr, len, encoding = 0) {
            const bytes = new Uint8Array(this.bridge.memory.buffer, Number(ptr), Number(len));
            const decoder = encoding === 1 ? new TextDecoder("utf-16le") : new TextDecoder();
            const str = decoder.decode(bytes);
            let handle = this.byString.get(str);
            if (handle === undefined) {
                handle = this.next;
                this.next += 1n;
                this.byString.set(str, handle);
                this.byHandle.set(handle, str);
            }
            return handle;
        }
        get(handle) {
            return this.byHandle.get(BigInt(handle));
        }
        drop(handle) {
            const key = BigInt(handle);
            const str = this.byHandle.get(key);
            if (str === undefined) return false;
            this.byHandle.delete(key);
            this.byString.delete(str);
            return true;
        }
    }
    class AnimationDriver {
        constructor(bridge, raf){
            this.bridge = bridge;
            this.raf = raf ?? defaultRaf();
            this.running = false;
            this.handle = null;
        }
        start() {
            if (this.running) return;
            this.running = true;
            this.#schedule();
        }
        #schedule() {
            this.handle = this.raf.request((ts)=>this.#frame(ts));
        }
        #frame(ts) {
            this.bridge.exports.trigger_animation_callbacks(ts);
            if (Number(this.bridge.exports.get_total_animation_callbacks()) > 0) {
                this.#schedule();
            } else {
                this.running = false;
                this.handle = null;
            }
        }
        stop() {
            if (this.handle !== null) this.raf.cancel(this.handle);
            this.running = false;
            this.handle = null;
        }
    }
    function defaultRaf() {
        if (typeof globalThis.requestAnimationFrame === "function") {
            return {
                request: (cb)=>globalThis.requestAnimationFrame(cb),
                cancel: (h)=>globalThis.cancelAnimationFrame?.(h)
            };
        }
        const now = ()=>globalThis.performance?.now?.() ?? Date.now();
        return {
            request: (cb)=>setTimeout(()=>cb(now()), 16),
            cancel: (h)=>clearTimeout(h)
        };
    }
    class TestReports {
        static PASS = 0;
        static FAIL = 1;
        static IGNORED = 2;
        constructor(){
            this.reports = [];
            this.waiters = [];
            this.last = null;
        }
        push(status, message) {
            const report = {
                status,
                message
            };
            const waiter = this.waiters.shift();
            if (waiter) {
                this.last = report;
                waiter(report);
            } else this.reports.push(report);
        }
        next() {
            const buffered = this.reports.shift();
            if (buffered) return Promise.resolve(buffered);
            this.last = null;
            return new Promise((resolve)=>this.waiters.push(resolve));
        }
        snapshot() {
            return this.last ? [
                ...this.reports,
                this.last
            ] : [
                ...this.reports
            ];
        }
        get pending() {
            return this.reports.length;
        }
    }
    function batchProtocolHandler(batches) {
        return {
            apply (_memoryId, payload) {
                if (payload.byteLength < 8) throw new Error("batch payload: missing header");
                const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
                const textsOff = view.getUint32(0, true);
                const textsLen = view.getUint32(4, true);
                if (textsOff < 8 || textsOff + textsLen > payload.byteLength) {
                    throw new Error("batch payload: header out of range");
                }
                const base = payload.byteOffset;
                batches.applyNoReturn(base + 8, textsOff - 8, base + textsOff, textsLen);
            }
        };
    }
    class AsyncTaskCollector {
        constructor(collect = false){
            this.tasks = [];
            this.collect = collect;
        }
        enable() {
            this.collect = true;
        }
        disable() {
            this.collect = false;
        }
        add(task) {
            if (!(task instanceof Promise)) throw new Error("Item must be a Promise");
            if (this.collect) this.tasks.push(task);
        }
        clear() {
            this.tasks.length = 0;
        }
        awaitAll() {
            return Promise.all(this.tasks);
        }
    }
    let _receiverIdCounter = 0;
    const _receivers = {};
    class WasmStreamReceiver {
        constructor(opts){
            this._id = ++_receiverIdCounter;
            this._onData = opts && opts.onData || function() {};
            this._onEnd = opts && opts.onEnd || function() {};
            this._onError = opts && opts.onError || function() {};
            _receivers[this._id] = this;
        }
        static _push(id, data, sequence, isLast) {
            var r = _receivers[id];
            if (!r) return;
            try {
                if (isLast) {
                    r._onEnd();
                    delete _receivers[id];
                } else {
                    r._onData({
                        data: data,
                        sequence: sequence
                    });
                }
            } catch (e) {
                r._onError(e);
            }
        }
        get receiverId() {
            return this._id;
        }
    }
    let _senderIdCounter = 0;
    const _senders = {};
    class WasmStreamSender {
        constructor(onChunk, onEnd){
            this._id = ++_senderIdCounter;
            this._onChunk = onChunk || function() {};
            this._onEnd = onEnd || function() {};
            _senders[this._id] = this;
        }
        static _send(id, data, sequence) {
            var s = _senders[id];
            if (s) s._onChunk({
                data: data,
                sequence: sequence
            });
        }
        static _end(id) {
            var s = _senders[id];
            if (s) {
                s._onEnd();
                delete _senders[id];
            }
        }
        get senderId() {
            return this._id;
        }
    }
    class FoundationWasm {
        constructor(opts = {}){
            this.bridge = {
                exports: null,
                memory: null
            };
            this.memory = new MemoryAllocations(this.bridge);
            this.timers = new TimerRegistry(this.bridge, opts.timerHost);
            this.callbacks = new CallbackRegistry(this.bridge, this.memory);
            this.animation = new AnimationDriver(this.bridge, opts.rafHost);
            this.strings = new StringCache(this.bridge);
            this.functions = new FunctionRegistry(this.bridge, this.memory, this.strings, this.callbacks);
            this.objects = new ExternalHeap();
            this.functions.reply.objects = this.objects;
            this.tasks = new AsyncTaskCollector(false);
            this.functions.tasks = this.tasks;
            this.testReports = new TestReports();
            this.batches = new BatchInstructions(this.bridge, this.memory, this.strings, this.functions, this.callbacks);
            this.dispatcher = new ProtocolDispatcher();
            this.dispatcher.setHandler(0, batchProtocolHandler(this.batches));
            this._capTriggerHandler = null;
            this._ipcTriggerHandler = null;
            this.dispatcher.setHandler(3, {
                apply: (mid, payload)=>this._dispatchCapTrigger(mid, payload)
            });
            this.dispatcher.setHandler(4, {
                apply: (mid, payload)=>this._dispatchIpcTrigger(mid, payload)
            });
            this._streamRegistry = null;
            this._streamRegistry = {};
            this._nextStreamId = 1;
        }
        triggerCapability(request) {
            if (!this._capTriggerHandler) {
                return Promise.reject(new Error('triggerCapability: no handler registered'));
            }
            return this._capTriggerHandler(request);
        }
        triggerIpc(request) {
            if (!this._ipcTriggerHandler) {
                return Promise.reject(new Error('triggerIpc: no handler registered'));
            }
            return this._ipcTriggerHandler(request);
        }
        registerTriggerHandlers(handlers) {
            if (handlers.onCapability) this._capTriggerHandler = handlers.onCapability;
            if (handlers.onIpc) this._ipcTriggerHandler = handlers.onIpc;
        }
        _dispatchCapTrigger(memoryId, payload) {
            if (!this._capTriggerHandler) return;
            try {
                var req = JSON.parse(new TextDecoder().decode(payload));
                this._capTriggerHandler(req);
            } catch (_) {}
            this.memory.dispose(memoryId);
        }
        _dispatchIpcTrigger(memoryId, payload) {
            if (!this._ipcTriggerHandler) return;
            try {
                var req = JSON.parse(new TextDecoder().decode(payload));
                this._ipcTriggerHandler(req);
            } catch (_) {}
            this.memory.dispose(memoryId);
        }
        get web_abi() {
            const self1 = this;
            const { bridge, memory, dispatcher, timers, animation, strings, functions, objects, batches } = this;
            return {
                host_apply (memId, ptr, len) {
                    try {
                        dispatcher.dispatch(memory.get(memId).bytes);
                    } finally{
                        memory.dispose(memId);
                    }
                },
                schedule_timeout (timing, callbackId) {
                    timers.scheduleTimeout(callbackId, Number(timing));
                },
                unschedule_timeout (callbackId) {
                    timers.cancelTimeout(callbackId);
                },
                schedule_microtask (callbackId) {
                    timers.scheduleMicrotask(callbackId);
                },
                schedule_interval (timing, callbackId) {
                    timers.scheduleInterval(callbackId, Number(timing));
                },
                unschedule_interval (callbackId) {
                    timers.cancelInterval(callbackId);
                },
                hook_up_animation_frames () {
                    animation.start();
                },
                host_cache_string (ptr, len, encoding) {
                    return strings.cache(ptr, len, Number(encoding));
                },
                host_report: (status, ptr, len)=>{
                    const length = Number(len);
                    const message = length ? new TextDecoder().decode(new Uint8Array(this.bridge.memory.buffer, Number(ptr), length)) : "";
                    this.testReports.push(Number(status), message);
                },
                host_monotonic_now_ms () {
                    return globalThis.performance?.now?.() ?? Date.now();
                },
                host_time_origin_ms () {
                    return globalThis.performance?.timeOrigin ?? 0;
                },
                host_wall_clock_ms () {
                    return Date.now();
                },
                host_fill_random: (ptr, len)=>{
                    const crypto = globalThis.crypto;
                    if (typeof crypto?.getRandomValues !== "function") return 1;
                    const MAX_PER_CALL = 65536;
                    const start = Number(ptr);
                    const total = Number(len);
                    try {
                        for(let done = 0; done < total; done += MAX_PER_CALL){
                            const n = Math.min(65536, total - done);
                            const buffer = this.bridge.memory.buffer;
                            const dest = new Uint8Array(buffer, start + done, n);
                            if (typeof SharedArrayBuffer !== "undefined" && buffer instanceof SharedArrayBuffer) {
                                const scratch = new Uint8Array(n);
                                crypto.getRandomValues(scratch);
                                dest.set(scratch);
                            } else {
                                crypto.getRandomValues(dest);
                            }
                        }
                    } catch (_) {
                        return 1;
                    }
                    return 0;
                },
                host_console_log (ptr, len) {
                    const length = Number(len);
                    const message = length ? new TextDecoder().decode(new Uint8Array(this.bridge.memory.buffer, Number(ptr), length)) : "";
                    console.log(message);
                },
                host_register_function (start, len, utf) {
                    return functions.register(start, len, utf);
                },
                host_invoke_function (handle, pPtr, pLen, rPtr, rLen) {
                    return functions.invoke(handle, pPtr, pLen, rPtr, rLen);
                },
                host_invoke_async_function (handle, callbackHandle, pPtr, pLen, rPtr, rLen) {
                    functions.invokeAsync(handle, callbackHandle, pPtr, pLen, rPtr, rLen);
                },
                host_invoke_function_as_bool: (h, p, l)=>functions.invokeAsBool(h, p, l),
                host_invoke_function_as_f32: (h, p, l)=>functions.invokeAsFloat(h, p, l),
                host_invoke_function_as_f64: (h, p, l)=>functions.invokeAsFloat(h, p, l),
                host_invoke_function_as_u8: (h, p, l)=>functions.invokeAsInt(h, p, l),
                host_invoke_function_as_u16: (h, p, l)=>functions.invokeAsInt(h, p, l),
                host_invoke_function_as_u32: (h, p, l)=>functions.invokeAsInt(h, p, l),
                host_invoke_function_as_i8: (h, p, l)=>functions.invokeAsInt(h, p, l),
                host_invoke_function_as_i16: (h, p, l)=>functions.invokeAsInt(h, p, l),
                host_invoke_function_as_i32: (h, p, l)=>functions.invokeAsInt(h, p, l),
                host_invoke_function_as_u64: (h, p, l)=>functions.invokeAsBigInt(h, p, l),
                host_invoke_function_as_object: (h, p, l)=>functions.invokeAsObject(h, p, l),
                host_invoke_function_as_i64: (h, p, l)=>functions.invokeAsBigInt(h, p, l),
                host_unregister_function (handle) {
                    functions.unregister(handle);
                },
                host_function_drop_external_pointer (handle) {
                    functions.unregister(handle);
                },
                host_invoke_function_as_str: (h, p, l)=>functions.invokeAsString(h, p, l),
                host_abort () {
                    throw new Error("WasmInstance called abort");
                },
                function_allocate_external_pointer: ()=>functions.allocate(),
                object_allocate_external_pointer: ()=>objects.create(null),
                host_object_drop_external_pointer (handle) {
                    objects.destroy(BigInt(handle));
                },
                host_string_cache_drop_external_pointer (handle) {
                    strings.drop(handle);
                },
                host_batch_apply (opsPtr, opsLen, textPtr, textLen) {
                    batches.applyNoReturn(opsPtr, opsLen, textPtr, textLen);
                },
                host_batch_returning_apply (opsPtr, opsLen, textPtr, textLen) {
                    return batches.applyReturning(opsPtr, opsLen, textPtr, textLen);
                },
                host_stream_create () {
                    var id = self1._nextStreamId++;
                    self1._streamRegistry[id] = {
                        chunks: [],
                        onChunk: null,
                        onEnd: null,
                        ended: false,
                        bind: function(chunkFn, endFn) {
                            this.onChunk = chunkFn || null;
                            this.onEnd = endFn || null;
                            if (this.onChunk) {
                                for(var i = 0; i < this.chunks.length; i++){
                                    this.onChunk(this.chunks[i]);
                                }
                                this.chunks.length = 0;
                            }
                            if (this.ended && this.onEnd) {
                                this.onEnd();
                            }
                        }
                    };
                    return BigInt(id);
                },
                host_stream_bind (streamId, cbOnChunk, cbOnEnd) {
                    var s = self1._streamRegistry[Number(streamId)];
                    if (!s) return;
                    s.onChunk = typeof cbOnChunk === "function" ? cbOnChunk : null;
                    s.onEnd = typeof cbOnEnd === "function" ? cbOnEnd : null;
                    if (s.onChunk) {
                        for(var i = 0; i < s.chunks.length; i++){
                            s.onChunk(s.chunks[i]);
                        }
                        s.chunks.length = 0;
                    }
                    if (s.ended && s.onEnd) {
                        s.onEnd();
                    }
                },
                host_sender_send (streamId, dataPtr, dataLen, seq) {
                    var id = Number(streamId);
                    var data = new Uint8Array(bridge.memory.buffer, Number(dataPtr), Number(dataLen));
                    var s = self1._streamRegistry[id];
                    if (s) {
                        var chunk = {
                            data: data,
                            sequence: Number(seq)
                        };
                        if (s.onChunk) {
                            s.onChunk(chunk);
                        } else {
                            s.chunks.push(chunk);
                        }
                    } else {
                        WasmStreamSender._send(id, data, Number(seq));
                    }
                },
                host_sender_end (streamId) {
                    var id = Number(streamId);
                    var s = self1._streamRegistry[id];
                    if (s) {
                        s.ended = true;
                        if (s.onEnd) s.onEnd();
                        delete self1._streamRegistry[id];
                    } else {
                        WasmStreamSender._end(id);
                    }
                },
                host_receiver_push (receiverId, dataPtr, dataLen, seq, isLast) {
                    var data = new Uint8Array(bridge.memory.buffer, Number(dataPtr), Number(dataLen));
                    WasmStreamReceiver._push(Number(receiverId), data, Number(seq), isLast !== 0);
                }
            };
        }
        init(moduleOrInstance) {
            const instance = moduleOrInstance.instance ?? moduleOrInstance;
            this.bridge.exports = instance.exports;
            this.bridge.memory = instance.exports.memory;
            return this;
        }
        dispose() {
            this.timers.cancelAll();
            this.animation.stop();
        }
        setProtocolHandler(protocol, handler) {
            this.dispatcher.setHandler(protocol, handler);
            return this;
        }
        awaitTasks() {
            return this.tasks.awaitAll();
        }
    }
    class WasmLoader {
        constructor(opts = {}){
            this.runtime = new FoundationWasm(opts);
            this.environment = opts.environment ?? {};
            this.compileOptions = WasmLoader.compileOptionsOf(opts.compileOptions);
            this.memory = new WebAssembly.Memory({
                initial: opts.initialMemory ?? 10,
                maximum: opts.maximumMemory ?? 200
            });
            this.module = null;
        }
        static compileOptionsOf(compileOptions) {
            return {
                builtins: compileOptions?.builtins ?? [
                    "js-strings"
                ],
                importedStringConstants: compileOptions?.importedStringConstants ?? "imported_strings"
            };
        }
        get imports() {
            return {
                abi: this.runtime.web_abi,
                js: {
                    mem: this.memory
                },
                ...this.environment
            };
        }
        async loadURL(url) {
            const module = await WebAssembly.instantiateStreaming(fetch(url), this.imports, this.compileOptions);
            this.#bind(module);
            return this;
        }
        async loadBytes(bytes) {
            const module = await WebAssembly.instantiate(bytes, this.imports, this.compileOptions);
            this.#bind(module);
            return this;
        }
        #bind(module) {
            this.module = module;
            this.runtime.init(module);
        }
        run() {
            if (!this.module) throw new Error("No wasm module loaded");
            const main = this.module.instance.exports.main;
            if (!main) throw new Error("wasm module has no exported main function");
            return main();
        }
        static async fromScripts(opts = {}) {
            const scripts = document.querySelectorAll('script[type="application/wasm"]');
            const loading = [];
            for (const script of scripts){
                if (!script.src) continue;
                loading.push(new WasmLoader(opts).loadURL(script.src));
            }
            return Promise.all(loading);
        }
    }
    class WasmWebScripts {
        constructor(opts = {}){
            this.modules = WasmLoader.fromScripts(opts);
        }
        static default(environment) {
            return new WasmWebScripts({
                environment
            });
        }
        async runAll() {
            const loaders = await this.modules;
            for (const loader of loaders)loader.run();
            return loaders;
        }
    }
    globalThis.FoundationWasmRuntime = Object.freeze({
        FoundationWasm,
        batchProtocolHandler,
        WasmLoader,
        WasmWebScripts,
        AsyncTaskCollector,
        TestReports,
        WasmEnvelope,
        ProtocolDispatcher,
        MemoryAllocations,
        TimerRegistry,
        CallbackRegistry,
        StringCache,
        AnimationDriver,
        FunctionRegistry,
        ParameterParser,
        ReturnHintParser,
        ReplyEncoder,
        ExternalHeap,
        BatchInstructions,
        BatchParameterParser,
        Operations,
        ArgumentOperations,
        TypeOptimization,
        ParamType,
        ReturnType,
        ReturnIds,
        ThreeStateId,
        ExternalPointer,
        InternalPointer,
        CachePointer,
        ErrorCodeValue,
        TypedArraySliceValue,
        TypedSliceArray,
        ReplyContainer,
        FakeNode,
        ReplyError,
        WasmStreamReceiver,
        WasmStreamSender
    });
    class ConnectionManager {
        #adapters = new Map();
        #connections = new Set();
        #closed = false;
        #tag = 0n;
        nextTag() {
            if (this.#tag === 0xffffffffffffffffn) throw new Error("Connection identifiers exhausted");
            return ++this.#tag;
        }
        supports(transport) {
            return this.#adapters.has(transport);
        }
        register(transport, open) {
            if (this.#closed || this.#adapters.has(transport)) throw new Error("Connection adapter is unavailable");
            this.#adapters.set(transport, open);
            return this;
        }
        acquire(options, listener = {}) {
            if (this.#closed) throw new Error("Connection manager is closed");
            const { transport, endpoint, shared = transport === "websocket", keepAlive = false, settings = null, authority = null, protocol = "frame" } = options;
            const url = new URL(endpoint, options.base ?? globalThis.location?.href);
            if (url.username || url.password) throw new Error("Use the connection authentication context");
            url.hash = "";
            if (transport === "websocket") {
                if (url.protocol === "https:") url.protocol = "wss:";
                if (url.protocol === "http:") url.protocol = "ws:";
            }
            let entry = shared && Array.from(this.#connections).find((c)=>c.shared && c.transport === transport && c.address === url.href && c.settings === settings && c.authority === authority && c.protocol === protocol);
            const handle = {
                ...listener
            };
            if (!entry) {
                const open = this.#adapters.get(transport);
                if (!open) throw new Error("Unknown connection transport: " + transport);
                entry = {
                    shared,
                    address: url.href,
                    settings,
                    authority,
                    transport,
                    protocol,
                    retainers: new Set(),
                    handles: new Set([
                        handle
                    ])
                };
                this.#connections.add(entry);
                try {
                    entry.connection = open({
                        ...options,
                        endpoint: url.href
                    }, {
                        listening: ()=>[
                                ...entry.handles
                            ].some((h)=>h.receive),
                        deliver: (bytes)=>{
                            if (!this.#connections.has(entry)) return;
                            if (options.isContext?.(bytes)) {
                                entry.context = new Uint8Array(bytes).slice();
                                for (const h of [
                                    ...entry.handles
                                ])h.onContext?.(entry.context.slice());
                                return;
                            }
                            const delivered = new Set();
                            for (const h of [
                                ...entry.handles
                            ]){
                                if (!entry.handles.has(h) || !h.receive || h.owner && delivered.has(h.owner)) continue;
                                try {
                                    const copy = new Uint8Array(bytes).slice();
                                    if (h.accept && !h.accept(copy)) continue;
                                    h.receive(copy);
                                    if (h.owner) delivered.add(h.owner);
                                } catch (cause) {
                                    h.onError?.(cause);
                                }
                            }
                        },
                        opened: ()=>{
                            entry.opened = true;
                            for (const h of [
                                ...entry.handles
                            ])h.onOpen?.();
                        },
                        disconnected: (cause)=>{
                            entry.opened = false;
                            entry.context = null;
                            for (const h of [
                                ...entry.handles
                            ])h.onError?.(cause);
                        }
                    });
                } catch (cause) {
                    this.#connections.delete(entry);
                    throw cause;
                }
            } else {
                entry.handles.add(handle);
                if (entry.opened || entry.context) queueMicrotask(()=>{
                    if (!entry.handles.has(handle)) return;
                    if (entry.opened) handle.onOpen?.();
                    if (entry.context) handle.onContext?.(entry.context.slice());
                });
            }
            const retention = {};
            if (keepAlive) entry.retainers.add(retention);
            const releaseUnused = ()=>{
                if (!entry.handles.size && !entry.retainers.size) this.#close(entry);
            };
            return {
                send: async (bytes)=>{
                    if (!entry.handles.has(handle) || !entry.connection?.send) throw new Error("Connection cannot send");
                    await entry.connection.send(bytes);
                },
                close: ()=>{
                    entry.handles.delete(handle);
                    releaseUnused();
                },
                releaseRetention: ()=>{
                    entry.retainers.delete(retention);
                    releaseUnused();
                }
            };
        }
        #close(entry) {
            if (!this.#connections.delete(entry)) return;
            entry.handles.clear();
            entry.connection?.close();
        }
        close() {
            this.#closed = true;
            for (const entry of [
                ...this.#connections
            ])this.#close(entry);
        }
    }
    const RESERVED = Object.freeze({
        HEAD: 0,
        BODY: 1,
        HTML: 2
    });
    class NodeRegistry {
        constructor(){
            this.nodes = new Map();
            this.pending = new Map();
        }
        seedDocument(doc) {
            if (doc.head) this.nodes.set(RESERVED.HEAD, doc.head);
            if (doc.body) this.nodes.set(RESERVED.BODY, doc.body);
            if (doc.documentElement) this.nodes.set(RESERVED.HTML, doc.documentElement);
            return this;
        }
        register(id, node) {
            this.nodes.set(id, node);
            return node;
        }
        get(id) {
            return this.nodes.get(id);
        }
        expect(id) {
            const node = this.nodes.get(id);
            if (node === undefined) throw new Error(`NodeRegistry: unknown node id ${id}`);
            return node;
        }
        unregister(id) {
            this.nodes.delete(id);
        }
        stage(id, node) {
            this.pending.set(id, node);
            return node;
        }
        applyRegister(id, document1) {
            if (this.nodes.has(id)) return this.nodes.get(id);
            const staged = this.pending.get(id);
            if (staged !== undefined) {
                this.pending.delete(id);
                return this.register(id, staged);
            }
            const found = document1.querySelector?.(`[primal-id="${id}"]`);
            if (found) return this.register(id, found);
            throw new Error(`NodeRegistry: REGISTER_NODE ${id} matches no staged node or [primal-id]`);
        }
        expectOrStaged(id) {
            const node = this.nodes.get(id);
            if (node !== undefined) return node;
            const staged = this.pending.get(id);
            if (staged !== undefined) {
                this.pending.delete(id);
                return staged;
            }
            throw new Error(`NodeRegistry: unknown node id ${id}`);
        }
        get size() {
            return this.nodes.size;
        }
    }
    const morphIsElement = (n)=>{
        if (!n) return false;
        if (n.nodeType !== undefined) return n.nodeType === 1;
        return typeof n.getAttributeNames === "function" && (n.tag ?? "") !== "#text";
    };
    const morphTag = (n)=>(n.tagName ?? n.tag ?? "").toUpperCase();
    const morphKids = (n)=>Array.from(n.childNodes ?? n.children ?? []);
    const morphText = (n)=>n.nodeValue !== undefined && n.nodeValue !== null ? n.nodeValue : n.textContent;
    const morphSetText = (n, v)=>{
        if (n.nodeValue !== undefined && n.nodeValue !== null) n.nodeValue = v;
        else n.textContent = v;
    };
    const morphAttr = (n, name)=>n.getAttribute ? n.getAttribute(name) : null;
    const morphIdentity = (n)=>morphAttr(n, "id") ?? morphAttr(n, "primal-id");
    function morphWalk(root, fn) {
        if (morphIsElement(root)) fn(root);
        for (const child of morphKids(root))morphWalk(child, fn);
    }
    function moveBefore(parent, node, ref) {
        if (typeof parent.moveBefore === "function") parent.moveBefore(node, ref);
        else {
            node.parentNode?.removeChild?.(node);
            parent.insertBefore(node, ref);
        }
    }
    class MorphContext {
        constructor(doc){
            this.doc = doc;
            this.idMap = new Map();
            this.persistentIds = new Set();
            this.oldIdTagMap = new Map();
            this.duplicates = new Set();
            this.pantry = null;
        }
        computePersistentIds(oldRoot, newRoot) {
            morphWalk(oldRoot, (el)=>{
                const id = morphIdentity(el);
                if (!id) return;
                if (this.oldIdTagMap.has(id)) this.duplicates.add(id);
                else this.oldIdTagMap.set(id, morphTag(el));
            });
            morphWalk(newRoot, (el)=>{
                const id = morphIdentity(el);
                if (!id || this.duplicates.has(id)) return;
                if (this.oldIdTagMap.get(id) === morphTag(el)) this.persistentIds.add(id);
            });
        }
        populateIdMap(root) {
            const build = (node)=>{
                const ids = new Set();
                const own = morphIsElement(node) ? morphIdentity(node) : null;
                if (own && this.persistentIds.has(own)) ids.add(own);
                for (const child of morphKids(node)){
                    for (const id of build(child))ids.add(id);
                }
                if (ids.size > 0) this.idMap.set(node, ids);
                return ids;
            };
            build(root);
        }
        hasConflictingId(node) {
            const id = morphIsElement(node) ? morphIdentity(node) : null;
            return !!id && !this.persistentIds.has(id);
        }
        findBestMatch(oldCursor, newChild) {
            if (!morphIsElement(newChild)) {
                return oldCursor && !morphIsElement(oldCursor) ? oldCursor : null;
            }
            const newIds = this.idMap.get(newChild) ?? new Set();
            if (newIds.size > 0) {
                const intersects = (candidate)=>{
                    if (!morphIsElement(candidate) || morphTag(candidate) !== morphTag(newChild) || !this.idMap.has(candidate)) {
                        return false;
                    }
                    const ids = this.idMap.get(candidate);
                    for (const id of newIds){
                        if (ids.has(id)) return true;
                    }
                    return false;
                };
                for(let candidate = oldCursor; candidate; candidate = candidate.nextSibling){
                    if (intersects(candidate)) return candidate;
                }
                if (this.pantry) {
                    for (const candidate of morphKids(this.pantry)){
                        if (intersects(candidate)) return candidate;
                    }
                }
            }
            if (oldCursor && morphIsElement(oldCursor) === morphIsElement(newChild) && morphTag(oldCursor) === morphTag(newChild) && !this.hasConflictingId(oldCursor)) {
                if (typeof oldCursor.isEqualNode === "function") {
                    if (oldCursor.isEqualNode(newChild)) return oldCursor;
                    const nextNew = newChild.nextSibling ?? null;
                    if (nextNew && oldCursor.isEqualNode(nextNew)) return null;
                }
                return oldCursor;
            }
            return null;
        }
        removeNode(node) {
            if (this.idMap.has(node)) {
                if (!this.pantry) this.pantry = this.doc.createElement("div");
                moveBefore(this.pantry, node, null);
            } else {
                node.parentNode?.removeChild?.(node);
            }
        }
        morphChildren(oldParent, newParent) {
            let oldCursor = oldParent.firstChild ?? null;
            for (const newChild of morphKids(newParent)){
                if (oldCursor && morphAttr(oldCursor, "data-ignore-morph") !== null && morphAttr(newChild, "data-ignore-morph") !== null) {
                    oldCursor = oldCursor.nextSibling;
                    continue;
                }
                const match = this.findBestMatch(oldCursor, newChild);
                if (match) {
                    while(oldCursor && oldCursor !== match){
                        const next = oldCursor.nextSibling;
                        this.removeNode(oldCursor);
                        oldCursor = next;
                    }
                    if (match !== oldCursor) moveBefore(oldParent, match, oldCursor);
                    this.morphNode(match, newChild);
                    oldCursor = match.nextSibling;
                } else {
                    const clone = cloneNode(this.doc, newChild);
                    oldParent.insertBefore(clone, oldCursor);
                }
            }
            while(oldCursor){
                const next = oldCursor.nextSibling;
                this.removeNode(oldCursor);
                oldCursor = next;
            }
        }
        morphNode(oldNode, newNode) {
            if (!morphIsElement(oldNode) || !morphIsElement(newNode)) {
                if (morphText(oldNode) !== morphText(newNode)) {
                    morphSetText(oldNode, morphText(newNode));
                }
                return;
            }
            if (typeof oldNode.isEqualNode === "function" && oldNode.isEqualNode(newNode)) {
                return;
            }
            this.syncAttributes(oldNode, newNode);
            preserveFormState(oldNode, newNode);
            const tag = morphTag(oldNode);
            if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
            this.morphChildren(oldNode, newNode);
        }
        syncAttributes(oldEl, newEl) {
            const preserved = (morphAttr(oldEl, "data-preserve-attr") || "").split(",").map((sliver)=>sliver.trim()).filter(Boolean);
            for (const name of newEl.getAttributeNames()){
                if (!preserved.includes(name)) oldEl.setAttribute(name, newEl.getAttribute(name));
            }
            for (const name of oldEl.getAttributeNames()){
                if (!preserved.includes(name) && newEl.getAttribute(name) === null) {
                    oldEl.removeAttribute(name);
                }
            }
        }
        cleanup() {
            if (this.pantry) {
                while(this.pantry.firstChild)this.pantry.removeChild(this.pantry.firstChild);
            }
            this.idMap.clear();
            this.persistentIds.clear();
            this.oldIdTagMap.clear();
            this.duplicates.clear();
        }
    }
    function preserveFormState(oldEl, newEl) {
        if (morphTag(oldEl) !== morphTag(newEl)) return;
        switch(morphTag(oldEl)){
            case "INPUT":
                {
                    const type = morphAttr(oldEl, "type");
                    if (type === "checkbox" || type === "radio") newEl.checked = oldEl.checked;
                    else if (type !== "file") newEl.value = oldEl.value;
                    break;
                }
            case "TEXTAREA":
                newEl.value = oldEl.value;
                break;
            case "SELECT":
                newEl.selectedIndex = oldEl.selectedIndex;
                break;
            default:
        }
    }
    function cloneNode(doc, node) {
        if (typeof node.cloneNode === "function") return node.cloneNode(true);
        if (!morphIsElement(node)) return doc.createTextNode(morphText(node) ?? "");
        const el = doc.createElement(node.tag ?? node.tagName);
        for (const name of node.getAttributeNames())el.setAttribute(name, node.getAttribute(name));
        if (node.value != null) el.value = node.value;
        if (node.checked != null) el.checked = node.checked;
        for (const child of morphKids(node))el.appendChild(cloneNode(doc, child));
        return el;
    }
    class MorphDom {
        static scripts = new WeakSet();
        static morph(target, newContent, doc = target.ownerDocument ?? (typeof document === "undefined" ? null : document)) {
            const ctx = new MorphContext(doc);
            try {
                ctx.computePersistentIds(target, newContent);
                ctx.populateIdMap(target);
                ctx.populateIdMap(newContent);
                ctx.morphChildren(target, newContent);
                MorphDom.executeNewScripts(target, doc);
            } finally{
                ctx.cleanup();
            }
        }
        static executeNewScripts(root, doc) {
            if (!doc || typeof root.querySelectorAll !== "function") return;
            for (const script of root.querySelectorAll("script")){
                if (MorphDom.scripts.has(script)) continue;
                const clone = doc.createElement("script");
                for (const name of script.getAttributeNames()){
                    clone.setAttribute(name, script.getAttribute(name));
                }
                clone.textContent = script.textContent;
                script.parentNode?.replaceChild?.(clone, script);
                MorphDom.scripts.add(clone);
            }
        }
    }
    const PRIMAL_ON = "primal:on";
    function buildEventData(eventType, event, el) {
        return {
            type: eventType,
            primalId: el.getAttribute ? el.getAttribute("primal-id") : null,
            value: el.value ?? null,
            checked: el.checked ?? null,
            keyCode: event.keyCode ?? null,
            modifiers: {
                alt: !!event.altKey,
                ctrl: !!event.ctrlKey,
                shift: !!event.shiftKey,
                meta: !!event.metaKey
            }
        };
    }
    function parseCallbackId(ref) {
        if (ref == null) return null;
        const s = String(ref).trim();
        const body = s.startsWith("callback-") ? s.slice("callback-".length) : s;
        return /^\d+$/.test(body) ? Number(body) : null;
    }
    class EventDispatcher {
        static actionHook = null;
        constructor(deliver, options = {}){
            this.deliver = deliver;
            this.deliverSignal = options.deliverSignal ?? deliver;
            this.scope = options.scope ?? globalThis;
            this.listeners = new WeakMap();
            this.cleanupQueue = [];
            this.cleanupScheduled = false;
            this.observer = null;
        }
        scanAndWire(root) {
            visit(root, (el)=>{
                for (const name of el.getAttributeNames()){
                    if (!name.startsWith(PRIMAL_ON)) continue;
                    const rest = name.slice(PRIMAL_ON.length);
                    const [eventType, mode] = rest.split(":");
                    if (!eventType) continue;
                    if (mode === "delegate") {
                        this.wireDelegated(el, eventType, el.getAttribute(name));
                    } else if (mode === undefined) {
                        this.wire(el, eventType, el.getAttribute(name));
                    }
                }
            });
        }
        wire(el, eventType, handlerRef) {
            const listener = this.#buildListener(el, eventType, handlerRef);
            if (!listener) {
                console.warn(`EventDispatcher: unresolvable handler "${handlerRef}" for ${eventType}`);
                return;
            }
            this.off(el, eventType);
            el.addEventListener(eventType, listener);
            this.#listenerMap(el).set(eventType, listener);
        }
        #buildListener(el, eventType, handlerRef) {
            if (/^(server|host):/.test(String(handlerRef))) {
                return (event)=>{
                    if (EventDispatcher.actionHook) return EventDispatcher.actionHook(String(handlerRef), el, event, eventType);
                    console.warn("No action runtime for", handlerRef);
                };
            }
            const callbackId = parseCallbackId(handlerRef);
            if (callbackId !== null) {
                return (event)=>this.deliver(callbackId, buildEventData(eventType, event, el));
            }
            const setterId = parseCallbackId(el.getAttribute?.("primal:setter"));
            if (setterId !== null) {
                return (event)=>this.deliverSignal(setterId, buildEventData(eventType, event, el));
            }
            const fn = resolveFunctionRef(this.scope, handlerRef);
            if (fn) return fn.bind(el);
            return null;
        }
        wireDelegated(el, eventType, selector) {
            const target = resolveDelegateTarget(el, selector, this.documentOf(el));
            if (!target) {
                console.warn(`EventDispatcher: delegate target "${selector}" not found`);
                return;
            }
            const listener = (event)=>{
                if (el === event.target || el.contains && el.contains(event.target)) {
                    event.delegateTarget = el;
                }
            };
            const key = `${eventType}:delegate:${el.getAttribute?.("primal-id") || el.id || ""}`;
            const map = this.#listenerMap(target);
            const prior = map.get(key);
            if (prior) target.removeEventListener(eventType, prior);
            map.set(key, listener);
            target.addEventListener(eventType, listener);
        }
        documentOf(el) {
            return el.ownerDocument ?? (typeof document === "undefined" ? null : document);
        }
        #listenerMap(el) {
            let map = this.listeners.get(el);
            if (!map) {
                map = new Map();
                this.listeners.set(el, map);
            }
            return map;
        }
        off(el, eventType) {
            const map = this.listeners.get(el);
            if (!map) return;
            if (eventType !== undefined) {
                for (const [key, listener] of map){
                    if (key === eventType || key.startsWith(`${eventType}:`)) {
                        el.removeEventListener(eventType, listener);
                        map.delete(key);
                    }
                }
                return;
            }
            for (const [key, listener] of map){
                el.removeEventListener(key.split(":")[0], listener);
            }
            map.clear();
            this.listeners.delete(el);
        }
        removeListeners(root) {
            visit(root, (el)=>this.off(el));
        }
        trackRemoved(node) {
            this.cleanupQueue.push(node);
            if (this.cleanupScheduled) return;
            this.cleanupScheduled = true;
            queueMicrotask(()=>{
                const batch = this.cleanupQueue.splice(0);
                this.cleanupScheduled = false;
                for (const item of batch)this.removeListeners(item);
            });
        }
        handleMutations(mutations) {
            for (const mutation of mutations){
                if (mutation.type !== "childList") continue;
                for (const node of mutation.addedNodes){
                    if (!isElement(node) || insideIsland(node)) continue;
                    this.scanAndWire(node);
                    Hydrator.hydrate(node);
                }
                for (const node of mutation.removedNodes){
                    if (!isElement(node) || insideIsland(node)) continue;
                    this.removeListeners(node);
                }
            }
        }
        observe(doc) {
            if (typeof MutationObserver === "undefined" || this.observer) return;
            this.observer = new MutationObserver((mutations)=>this.handleMutations(mutations));
            this.observer.observe(doc, {
                subtree: true,
                childList: true
            });
        }
        on(el, eventType, handlerRef, options = {}) {
            if (options.delegate) this.wireDelegated(el, eventType, options.delegate);
            else this.wire(el, eventType, handlerRef);
        }
    }
    const CONVENIENCE_EVENTS = [
        "click",
        "change",
        "submit",
        "keydown",
        "keyup",
        "focus",
        "blur",
        "scroll",
        "input",
        "mousedown",
        "mouseup"
    ];
    for (const evt of CONVENIENCE_EVENTS){
        EventDispatcher.prototype[`on${evt}`] = function(el, handlerRef, options) {
            this.on(el, evt, handlerRef, options);
        };
    }
    function visit(node, fn) {
        if (typeof node.getAttributeNames === "function") fn(node);
        for (const child of node.children || [])visit(child, fn);
    }
    function isElement(node) {
        return !!node && typeof node.getAttributeNames === "function";
    }
    function insideIsland(node) {
        if (typeof node.closest === "function") return node.closest("island") !== null;
        for(let cur = node; cur; cur = cur.parent ?? cur.parentElement ?? null){
            if ((cur.tag ?? cur.tagName ?? "").toLowerCase() === "island") return true;
        }
        return false;
    }
    function resolveFunctionRef(scope, handlerRef) {
        if (typeof handlerRef !== "string" || handlerRef.length === 0) return null;
        let current = scope;
        for (const part of handlerRef.split(".")){
            if (current == null) return null;
            current = current[part];
        }
        return typeof current === "function" ? current : null;
    }
    function resolveDelegateTarget(el, selector, doc) {
        if (!selector) return null;
        if (selector === "parent") return el.parentElement ?? el.parent ?? null;
        if (selector === "body") return doc?.body ?? null;
        return doc?.querySelector ? doc.querySelector(selector) : null;
    }
    function initEventRuntime(dispatcher, doc = typeof document === "undefined" ? null : document) {
        if (!doc) return;
        const boot = ()=>{
            dispatcher.scanAndWire(doc.body);
            dispatcher.observe(doc);
        };
        doc.addEventListener?.("DOMContentLoaded", boot);
        if (doc.readyState !== "loading") boot();
    }
    function callbackDeliver(callbackRegistry, encode = jsonEncodeEventData) {
        return (callbackId, eventData)=>callbackRegistry.invoke(callbackId, encode(eventData));
    }
    function signalDeliver(bridge, encode = jsonEncodeEventData) {
        return (setterId, eventData)=>{
            const bytes = encode(eventData);
            const memId = bridge.exports.create_allocation(BigInt(bytes.length));
            const ptr = Number(bridge.exports.allocation_start_pointer(memId));
            const memory = typeof bridge.memory === "function" ? bridge.memory() : bridge.memory;
            new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
            bridge.exports.invoke_signal_callback(BigInt(setterId), memId);
        };
    }
    function jsonEncodeEventData(eventData) {
        return new TextEncoder().encode(JSON.stringify(eventData));
    }
    class DomHeap {
        static RESERVED_SLOTS = 5;
        constructor(host = globalThis){
            this.items = [];
            this.free = [];
            const doc = host.document ?? null;
            this.create(typeof self !== "undefined" ? self : this);
            this.create(this);
            this.create(host.window ?? null);
            this.create(doc);
            this.create(doc && doc.body ? doc.body : null);
        }
        #unpack(uid) {
            const v = BigInt(uid);
            if (v < BigInt(DomHeap.RESERVED_SLOTS)) return {
                index: Number(v),
                generation: 0n
            };
            return {
                index: Number(v >> 32n),
                generation: v & 0xffffffffn
            };
        }
        create(item) {
            let index;
            if (this.free.length > 0) {
                index = this.free.pop();
                const slot = this.items[index];
                slot.generation += 1n;
                slot.active = true;
                slot.item = item;
            } else {
                index = this.items.length;
                this.items.push({
                    item,
                    generation: 0n,
                    active: true
                });
            }
            return BigInt(index) << 32n | this.items[index].generation;
        }
        get(uid) {
            const { index, generation } = this.#unpack(uid);
            const slot = this.items[index];
            if (!slot || !slot.active || slot.generation !== generation) return undefined;
            return slot.item;
        }
        update(uid, item) {
            const { index, generation } = this.#unpack(uid);
            const slot = this.items[index];
            if (!slot || slot.generation !== generation) return false;
            slot.item = item;
            slot.active = true;
            return true;
        }
        destroy(uid) {
            const { index, generation } = this.#unpack(uid);
            if (index < DomHeap.RESERVED_SLOTS) return false;
            const slot = this.items[index];
            if (!slot || !slot.active || slot.generation !== generation) return false;
            slot.item = null;
            slot.active = false;
            this.free.push(index);
            return true;
        }
    }
    function domAbi(rt, dom) {
        rt.functions.reply.dom = dom;
        return {
            dom_allocate_external_pointer: ()=>dom.create(null),
            host_dom_drop_external_pointer: (handle)=>{
                dom.destroy(BigInt(handle));
            },
            host_invoke_function_as_dom: (h, p, l)=>{
                const v = rt.functions.invokeNakedAs(h, p, l, 29);
                return typeof v === "bigint" ? v : BigInt(v);
            }
        };
    }
    function reconnectDelay(attempt) {
        return Math.min(1000 * 2 ** attempt, 30_000);
    }
    async function probeBatching(fetchFn = globalThis.fetch) {
        try {
            const res = await fetchFn("/primal/messages", {
                method: "HEAD"
            });
            return !!res && res.ok === true;
        } catch  {
            return false;
        }
    }
    class RequestQueue {
        constructor(transport){
            this.queue = [];
            this.scheduled = false;
            this.transport = transport;
            this.bundlingEnabled = false;
            this.nextId = 1;
        }
        async probe(fetchFn = globalThis.fetch) {
            this.bundlingEnabled = await probeBatching(fetchFn);
            return this.bundlingEnabled;
        }
        enqueue(request) {
            if (!this.bundlingEnabled) {
                return this.transport.send(request.url, request.method, request.data);
            }
            this.queue.push({
                id: this.nextId++,
                url: request.url,
                method: request.method ?? "GET",
                headers: request.headers,
                body: request.data
            });
            if (!this.scheduled) {
                this.scheduled = true;
                queueMicrotask(()=>this.flush());
            }
            return null;
        }
        flush() {
            this.scheduled = false;
            if (this.queue.length === 0) return;
            const batch = this.queue.splice(0);
            if (batch.length === 1) {
                const only = batch[0];
                this.transport.send(only.url, only.method, only.body);
                return;
            }
            this.transport.send("/primal/messages", "POST", batch);
        }
    }
    function parseBatchEntry(entry) {
        const ct = entry.headers?.["content-type"] ?? entry.headers?.["Content-Type"] ?? "";
        if (ct.includes("primal-json")) return {
            id: entry.id,
            ...routeJson(entry.body)
        };
        if (ct.includes("primal-html") || ct.includes("text/html")) {
            return {
                id: entry.id,
                ...routeHtml(String(entry.body))
            };
        }
        if (ct.includes("primal-arrow")) {
            return {
                id: entry.id,
                ...removedArrow()
            };
        }
        return {
            id: entry.id,
            type: "raw",
            text: String(entry.body ?? "")
        };
    }
    class WSBatchQueue {
        constructor(ws){
            this.ws = ws;
            this.queue = [];
            this.scheduled = false;
        }
        send(data) {
            this.queue.push(data);
            if (!this.scheduled) {
                this.scheduled = true;
                queueMicrotask(()=>this.flush());
            }
        }
        flush() {
            this.scheduled = false;
            if (this.queue.length === 1) {
                this.ws.send(JSON.stringify(this.queue.splice(0, 1)[0]));
            } else if (this.queue.length > 1) {
                this.ws.send(JSON.stringify({
                    batch: this.queue.splice(0)
                }));
            }
        }
    }
    class WorkerBatchQueue {
        constructor(worker){
            this.worker = worker;
            this.queue = [];
            this.scheduled = false;
        }
        postMessage(data) {
            this.queue.push(data);
            if (!this.scheduled) {
                this.scheduled = true;
                queueMicrotask(()=>this.flush());
            }
        }
        flush() {
            this.scheduled = false;
            if (this.queue.length === 0) return;
            const batch = this.queue.splice(0);
            const transfers = batch.map((d)=>d?.transferable).filter(Boolean);
            this.worker.postMessage({
                batch
            }, transfers);
        }
    }
    class FetchTransport {
        constructor(config = {}){
            this.config = config;
            this.fetchFn = config.fetchFn ?? ((...args)=>globalThis.fetch(...args));
        }
        async send(url, method = "POST", data = undefined) {
            const init = {
                method,
                headers: this.config.headers
            };
            if (data !== undefined && method !== "GET" && method !== "HEAD") {
                init.body = typeof data === "string" ? data : JSON.stringify(data);
            }
            const response = await this.fetchFn(url, init);
            if (!response.ok) throw new Error(`FetchTransport: ${method} ${url} -> ${response.status}`);
            const handler = ProtocolHandler.named(this.config.protocol) ?? ProtocolHandler.fromContentType(response);
            return handler.process(response);
        }
        disconnect() {}
    }
    class SseParser {
        constructor(){
            this.decoder = new TextDecoder("utf-8");
            this.textBuffer = "";
            this.sawCarriageReturn = false;
            this.lastEventId = null;
            this.#resetBuilder();
        }
        #resetBuilder() {
            this.id = null;
            this.eventType = null;
            this.data = [];
            this.retry = null;
        }
        push(chunk) {
            let text = this.decoder.decode(chunk, {
                stream: true
            });
            if (this.sawCarriageReturn) {
                this.sawCarriageReturn = false;
                if (text.startsWith("\n")) text = text.slice(1);
            }
            if (text.endsWith("\r")) {
                this.sawCarriageReturn = true;
            }
            this.textBuffer += text;
            const events = [];
            for(;;){
                const cut = this.#nextLineEnd();
                if (cut === null) break;
                const [line, rest] = cut;
                this.textBuffer = rest;
                const event = this.#processLine(line);
                if (event) events.push(event);
            }
            return events;
        }
        end() {
            const tail = this.decoder.decode();
            if (tail) this.textBuffer += tail;
            const events = [];
            if (this.textBuffer.length > 0) {
                const event = this.#processLine(this.textBuffer.replace(/\r$/, ""));
                this.textBuffer = "";
                if (event) events.push(event);
            }
            const flushed = this.#dispatch();
            if (flushed) events.push(flushed);
            return events;
        }
        #nextLineEnd() {
            const buf = this.textBuffer;
            for(let i = 0; i < buf.length; i++){
                const ch = buf[i];
                if (ch === "\n") return [
                    buf.slice(0, i),
                    buf.slice(i + 1)
                ];
                if (ch === "\r") {
                    if (i + 1 < buf.length) {
                        const skip = buf[i + 1] === "\n" ? 2 : 1;
                        return [
                            buf.slice(0, i),
                            buf.slice(i + skip)
                        ];
                    }
                    return null;
                }
            }
            return null;
        }
        #processLine(line) {
            if (line.length === 0) {
                return this.#dispatch();
            }
            if (line.startsWith(":")) {
                return {
                    type: "comment",
                    comment: line.slice(1).replace(/^ /, "").trimStart(),
                    lastEventId: this.lastEventId
                };
            }
            const colon = line.indexOf(":");
            if (colon === -1) return null;
            const field = line.slice(0, colon);
            let value = line.slice(colon + 1);
            if (value.startsWith(" ")) value = value.slice(1);
            switch(field){
                case "id":
                    if (!value.includes("\0")) this.id = value;
                    break;
                case "event":
                    this.eventType = value;
                    break;
                case "data":
                    this.data.push(value);
                    break;
                case "retry":
                    {
                        const ms = Number(value);
                        if (Number.isInteger(ms) && ms >= 0 && /^\d+$/.test(value)) this.retry = ms;
                        break;
                    }
                default:
            }
            return null;
        }
        #dispatch() {
            if (this.data.length === 0) {
                this.#resetBuilder();
                return null;
            }
            if (this.id !== null) this.lastEventId = this.id;
            const event = {
                type: "message",
                event: this.eventType,
                data: this.data.join("\n"),
                id: this.id,
                lastEventId: this.lastEventId,
                retry: this.retry
            };
            this.#resetBuilder();
            return event;
        }
    }
    class FetchEventSource {
        constructor(url, options = {}){
            this.url = url;
            this.options = options;
            this.fetchFn = options.fetchFn ?? ((...args)=>globalThis.fetch(...args));
            this.closed = false;
            this.attempt = 0;
            this.retryOverride = null;
            this.lastEventId = null;
            this.abort = null;
        }
        async connect() {
            this.closed = false;
            await this.#attemptOnce();
        }
        async #attemptOnce() {
            if (this.closed) return;
            this.abort = typeof AbortController === "undefined" ? null : new AbortController();
            const headers = {
                accept: "text/event-stream",
                ...this.options.headers ?? {}
            };
            if (this.lastEventId !== null) headers["last-event-id"] = this.lastEventId;
            let response;
            try {
                response = await this.fetchFn(this.options.nextUrl?.(this.url) ?? this.url, {
                    method: this.options.method ?? "GET",
                    cache: "no-store",
                    headers,
                    body: this.options.body,
                    signal: this.abort?.signal
                });
                if (response.status === 410 && this.options.onExpired) {
                    this.close();
                    this.options.onExpired();
                    return;
                }
                if (!response.ok) throw new Error(`FetchEventSource: ${response.status}`);
            } catch (error) {
                this.options.onError?.(error);
                this.#scheduleReconnect();
                return;
            }
            this.options.onOpen?.(response);
            this.attempt = 0;
            const parser = new SseParser();
            const reader = response.body.getReader();
            try {
                for(;;){
                    const { done, value } = await reader.read();
                    if (done) break;
                    for (const event of parser.push(value))this.#deliver(event);
                }
                for (const event of parser.end())this.#deliver(event);
            } catch (error) {
                if (!this.closed) this.options.onError?.(error);
            }
            this.lastEventId = parser.lastEventId ?? this.lastEventId;
            this.#scheduleReconnect();
        }
        #deliver(event) {
            if (event.type === "comment") {
                this.options.onComment?.(event);
                return;
            }
            if (event.retry !== null && event.retry !== undefined) {
                this.retryOverride = event.retry;
            }
            if (event.id !== null) this.lastEventId = event.id;
            this.options.onEvent?.(event);
        }
        #scheduleReconnect() {
            if (this.closed) return;
            const delay = this.retryOverride ?? reconnectDelay(this.attempt);
            this.attempt += 1;
            this.reconnectTimer = setTimeout(()=>{
                if (!this.closed) this.#attemptOnce();
            }, delay);
        }
        close() {
            this.closed = true;
            clearTimeout(this.reconnectTimer);
            this.abort?.abort?.();
        }
    }
    class SSETransport {
        constructor(config = {}){
            this.config = config;
            this.source = null;
        }
        connect(url, data, onResult) {
            const method = this.config.method ?? (data === undefined ? "GET" : "POST");
            this.source = new FetchEventSource(url, {
                method,
                headers: {
                    ...data === undefined ? {} : {
                        "content-type": "application/json"
                    },
                    ...this.config.headers ?? {}
                },
                body: data === undefined ? undefined : JSON.stringify(data),
                fetchFn: this.config.fetchFn,
                onEvent: (event)=>{
                    if (this.config.listening?.() !== false) onResult(streamEventResult(this.config.protocol ?? event.event ?? "html", event.data));
                },
                onOpen: this.config.onOpen,
                onError: this.config.onError,
                nextUrl: this.config.nextUrl,
                onExpired: this.config.onExpired
            });
            return this.source.connect();
        }
        disconnect() {
            this.source?.close?.();
            this.source = null;
        }
    }
    class WebSocketTransport {
        constructor(config = {}){
            this.config = config;
            this.ws = null;
            this.attempt = 0;
            this.closed = false;
        }
        connect(url, _data, onResult) {
            const factory = this.config.wsFactory ?? (typeof WebSocket === "undefined" ? null : (target)=>new WebSocket(target));
            if (!factory) {
                throw new Error("WebSocketTransport: WebSocket unavailable in this environment");
            }
            this.closed = false;
            this.url = url;
            this.onResult = onResult;
            const socket = factory(this.config.nextUrl?.(url) ?? url);
            this.ws = socket;
            if ("binaryType" in this.ws) this.ws.binaryType = "arraybuffer";
            this.ws.onopen = ()=>{
                if (this.ws !== socket || this.closed) return;
                this.attempt = 0;
                this.config.onOpen?.();
            };
            this.ws.onmessage = (event)=>{
                if (this.ws !== socket || this.closed || this.config.listening?.() === false) return;
                const { data } = event;
                if (data instanceof ArrayBuffer || ArrayBuffer.isView(data)) {
                    const result = decodeEnvelopeFrame(data instanceof ArrayBuffer ? new Uint8Array(data) : new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
                    if (result) onResult(result);
                    return;
                }
                onResult(streamEventResult(this.config.protocol ?? "json", data));
            };
            this.ws.onclose = ()=>{
                if (this.closed || this.ws !== socket) return;
                this.config.onError?.(new Error("WebSocket disconnected"));
                const delay = reconnectDelay(this.attempt);
                this.attempt += 1;
                setTimeout(()=>{
                    if (!this.closed && this.ws === socket) this.connect(this.url, undefined, this.onResult);
                }, delay);
            };
        }
        send(data) {
            if (!this.ws || this.ws.readyState !== undefined && this.ws.readyState !== 1) throw new Error("WebSocket is closed");
            this.ws.send(typeof data === "string" || ArrayBuffer.isView(data) || data instanceof ArrayBuffer ? data : JSON.stringify(data));
        }
        disconnect() {
            this.closed = true;
            this.ws?.close?.();
            this.ws = null;
        }
    }
    class ChunkedTransport {
        constructor(config = {}){
            this.config = config;
            this.abort = null;
            this.fetchFn = config.fetchFn ?? ((...args)=>globalThis.fetch(...args));
        }
        async connect(url, data, onChunk) {
            this.abort = new AbortController();
            const response = await this.fetchFn(url, {
                method: data === undefined ? "GET" : "POST",
                body: data === undefined ? undefined : JSON.stringify(data),
                signal: this.abort.signal,
                headers: this.config.headers
            });
            if (response.status === 410 && this.config.onExpired) {
                this.config.onExpired();
                return;
            }
            if (!response.ok) throw new Error("Stream request failed with HTTP " + response.status);
            this.config.onOpen?.();
            const handler = ProtocolHandler.named(this.config.protocol) ?? ProtocolHandler.fromContentType(response);
            if (handler instanceof FrameHandler && isViewResponse(response)) {
                onChunk(await handler.process(response));
            } else if (handler instanceof FrameHandler) {
                const hook = ProtocolHandler.frameHook;
                if (!hook) throw new Error("Install the frame runtime before opening a frame stream");
                for await (const frame of hook.readFrames(response.body, hook.maxFrameBytes)){
                    if (this.config.listening?.() !== false) onChunk({
                        type: "frame",
                        frames: [
                            frame
                        ]
                    });
                }
            } else {
                const limit = this.config.maxDocumentBytes ?? ProtocolHandler.frameHook?.maxFrameBytes ?? 4 * 1024 * 1024;
                let size = 0;
                const body = response.body.pipeThrough(new TransformStream({
                    transform (bytes, controller) {
                        size += bytes.byteLength;
                        if (size > limit) throw new RangeError("Streamed document exceeds byte limit");
                        controller.enqueue(bytes);
                    }
                }));
                onChunk(await handler.process(new Response(body, {
                    headers: response.headers
                })));
            }
        }
        disconnect() {
            this.abort?.abort?.();
            this.abort = null;
        }
    }
    class WorkerTransport {
        constructor(config = {}){
            this.worker = config.worker;
        }
        send(_url, _method, data) {
            this.worker.postMessage(data);
        }
        onMessage(callback) {
            this.worker.onmessage = (event)=>callback(event.data);
        }
        disconnect() {
            this.worker?.terminate?.();
        }
    }
    class Transport {
        static create(config = {}) {
            switch(config.transport){
                case "sse":
                    return new SSETransport(config);
                case "ws":
                case "websocket":
                    return new WebSocketTransport(config);
                case "chunked":
                    return new ChunkedTransport(config);
                case "worker":
                    return new WorkerTransport(config);
                default:
                    return new FetchTransport(config);
            }
        }
    }
    function frameConnections(config = {}) {
        const manager = new ConnectionManager();
        for (const transport of [
            "sse",
            "websocket",
            "chunked"
        ])manager.register(transport, (options, events)=>{
            const channel = options.channel ?? Transport.create({
                ...config,
                headers: {
                    ...config.headers,
                    ...options.headers
                },
                transport,
                protocol: "frame"
            });
            let waiting = [];
            const opened = ()=>{
                for (const pending of waiting.splice(0))pending.resolve();
                events.opened();
            };
            const disconnected = (cause)=>{
                for (const pending of waiting.splice(0))pending.reject(cause);
                events.disconnected(cause);
            };
            if (channel.config) Object.assign(channel.config, {
                onOpen: opened,
                onError: disconnected,
                listening: events.listening
            });
            const result = channel.connect(options.endpoint, options.data, (message)=>{
                if (message.type !== "frame") {
                    disconnected(new Error("Expected a Foundation frame"));
                    return;
                }
                for (const frame of message.frames)events.deliver(frame);
            });
            Promise.resolve(result).catch(disconnected);
            return {
                send: transport === "websocket" ? async (bytes)=>{
                    if (channel.ws?.readyState === 0) await new Promise((resolve, reject)=>waiting.push({
                            resolve,
                            reject
                        }));
                    channel.send(bytes);
                } : undefined,
                close () {
                    for (const pending of waiting.splice(0))pending.reject(new Error("Connection closed"));
                    channel.disconnect();
                }
            };
        });
        return manager;
    }
    function decodeEnvelopeFrame(bytes) {
        if (ProtocolHandler.frameHook?.isFrame(bytes)) return {
            type: "frame",
            frames: [
                bytes
            ]
        };
        if (!(bytes instanceof Uint8Array)) bytes = new Uint8Array(bytes);
        if (bytes.byteLength < 6) {
            console.error("decodeEnvelopeFrame: truncated header", bytes.byteLength);
            return null;
        }
        const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
        const protocol = view.getUint8(0);
        const version = view.getUint8(1);
        const length = view.getUint32(2, true);
        const payload = bytes.subarray(6);
        if (payload.byteLength !== length) {
            console.error(`decodeEnvelopeFrame: length mismatch (header ${length}, payload ${payload.byteLength})`);
            return null;
        }
        if (protocol === 2) {
            try {
                return routeJson(JSON.parse(new TextDecoder().decode(payload)));
            } catch (error) {
                console.error("decodeEnvelopeFrame: json payload did not parse", error);
                return null;
            }
        }
        console.error(`decodeEnvelopeFrame: unknown protocol ${protocol} v${version}`);
        return null;
    }
    function base64ToBytes(text) {
        const bin = atob(String(text).trim());
        const bytes = new Uint8Array(bin.length);
        for(let i = 0; i < bin.length; i += 1)bytes[i] = bin.charCodeAt(i);
        return bytes;
    }
    function streamEventResult(kind, raw) {
        if (kind === "frame") return {
            type: "frame",
            frames: [
                base64ToBytes(raw)
            ]
        };
        if (kind === "json") {
            try {
                return routeJson(JSON.parse(raw));
            } catch  {
                return {
                    type: "raw",
                    text: String(raw)
                };
            }
        }
        if (kind === "arrow") return removedArrow();
        return routeHtml(String(raw));
    }
    function routeJson(value) {
        if (value && typeof value === "object" && value.morph) {
            return {
                type: "json-morph",
                morph: value.morph
            };
        }
        return {
            type: "json",
            patches: value
        };
    }
    function routeHtml(html) {
        const match = /^\s*<island\b[^>]*data-target="([^"]+)"[^>]*data-action="([^"]+)"[^>]*>([\s\S]*)<\/island>\s*$/.exec(html);
        if (match) {
            return {
                type: "html-morph",
                target: match[1],
                action: match[2],
                content: match[3]
            };
        }
        return {
            type: "html",
            html
        };
    }
    function removedArrow() {
        console.error('mount: protocol "arrow" was removed; use "frame"');
        return {
            type: "raw",
            text: ""
        };
    }
    const emptyProtocolHandler = {
        process: async ()=>({
                type: "raw",
                text: ""
            })
    };
    class JsonHandler {
        async process(response) {
            return routeJson(await response.json());
        }
    }
    class HtmlHandler {
        async process(response) {
            return routeHtml(await response.text());
        }
    }
    class RawHandler {
        async process(response) {
            return {
                type: "raw",
                text: await response.text()
            };
        }
    }
    function isViewResponse(response) {
        return response.headers?.get?.("content-type")?.split(";")[0].trim().toLowerCase() === "application/vnd.ewe.frame";
    }
    class FrameHandler {
        async process(response) {
            if (!response.ok) throw new Error("Frame request failed with HTTP " + response.status);
            const hook = ProtocolHandler.frameHook;
            if (!hook) {
                console.error("frame received but the frame runtime is not installed");
                return {
                    type: "raw",
                    text: ""
                };
            }
            if (isViewResponse(response)) {
                return {
                    type: "frame",
                    frames: [
                        await hook.readViewResponse(response, hook.maxFrameBytes)
                    ]
                };
            }
            const frames = [];
            let total = 0;
            for await (const frame of hook.readFrames(response.body, hook.maxFrameBytes)){
                total += frame.length;
                if (total > hook.maxFrameBytes) throw new RangeError("Frame reply exceeds byte limit");
                frames.push(frame);
            }
            return {
                type: "frame",
                frames
            };
        }
    }
    class ProtocolHandler {
        static frameHook = null;
        static named(name) {
            switch(name){
                case "frame":
                    return new FrameHandler();
                case "arrow":
                    removedArrow();
                    return emptyProtocolHandler;
                case "json":
                    return new JsonHandler();
                case "html":
                    return new HtmlHandler();
                default:
                    return null;
            }
        }
        static fromContentType(response) {
            const ct = response.headers?.get?.("content-type") || "";
            if (isViewResponse(response) || ct.includes("primal-frame")) return new FrameHandler();
            if (ct.includes("primal-arrow") || ct.includes("event-stream-arrow")) return ProtocolHandler.named("arrow");
            if (ct.includes("primal-json") || ct.includes("event-stream-json")) return new JsonHandler();
            if (ct.includes("primal-html") || ct.includes("event-stream-html") || ct.includes("text/html")) {
                return new HtmlHandler();
            }
            return new RawHandler();
        }
    }
    function scopeCss(cssText, prefix) {
        return cssText.replace(/(^|\})\s*([^@{}][^{}]*)\{/g, (_, brace, selectors)=>{
            const scoped = selectors.split(",").map((sel)=>`${prefix} ${sel.trim()}`).join(", ");
            return `${brace}\n${scoped} {`;
        });
    }
    class Hydrator {
        static scopeExtensions = [];
        static behaviourMarkers = null;
        static createScope(targetElement) {
            const eventCleanups = [];
            const scope = {
                targets: ()=>targetElement.querySelectorAll("[primal-id]"),
                parent: ()=>targetElement,
                querySelector: (sel)=>targetElement.querySelector(sel),
                querySelectorAll: (sel)=>targetElement.querySelectorAll(sel),
                addEvent (target, event, handler, options) {
                    const el = typeof target === "string" ? targetElement.querySelector(target) : target;
                    if (el) {
                        el.addEventListener(event, handler, options);
                        eventCleanups.push(()=>el.removeEventListener(event, handler, options));
                    }
                },
                onDispose (dispose) {
                    eventCleanups.push(dispose);
                },
                cleanup () {
                    for (const cleanup of eventCleanups.splice(0))cleanup();
                }
            };
            for (const extend of Hydrator.scopeExtensions)extend(scope, targetElement);
            return scope;
        }
        static hydrate(root, doc = root.ownerDocument ?? globalThis.document) {
            if (typeof root.querySelectorAll !== "function") return;
            Hydrator.behaviourMarkers?.(root);
            for (const style of root.querySelectorAll("style[primal\\:style]")){
                const prefix = root.id ? `#${root.id}` : `[primal-id="${root.getAttribute?.("primal-id") ?? ""}"]`;
                const scoped = scopeCss(style.textContent, prefix);
                if (typeof CSSStyleSheet === "function" && doc?.adoptedStyleSheets) {
                    const sheet = new CSSStyleSheet();
                    sheet.replaceSync(scoped);
                    doc.adoptedStyleSheets = [
                        ...doc.adoptedStyleSheets,
                        sheet
                    ];
                    (root._sheets ??= []).push(sheet);
                }
                style.remove?.();
            }
            for (const script of root.querySelectorAll("script[primal\\:script]")){
                const owner = script.parentElement ?? root;
                const scope = Hydrator.createScope(owner);
                try {
                    const source = script.textContent.trim();
                    const body = /^(?:async\s+)?function\s*\(/.test(source) ? `(${source})(scope)` : source;
                    new Function("scope", body)(scope);
                    Patcher.registerUnmount(owner, ()=>scope.cleanup());
                } catch (error) {
                    scope.cleanup();
                    console.error("Hydrator: scoped script failed", error);
                }
                script.remove?.();
            }
        }
    }
    class Patcher {
        static runtime = {
            dispatcher: null,
            registry: null,
            signalBridge: null,
            frames: null
        };
        static frameSeq = 0;
        static localScopes = new WeakMap();
        static registerUnmount(node, dispose) {
            let hooks = Patcher.localScopes.get(node);
            if (!hooks) Patcher.localScopes.set(node, hooks = new Set());
            hooks.add(dispose);
        }
        static releaseScope(node) {
            const hooks = Patcher.localScopes.get(node);
            Patcher.localScopes.delete(node);
            if (hooks) for (const dispose of hooks){
                try {
                    dispose();
                } catch (error) {
                    console.error("Component cleanup failed", error);
                }
            }
        }
        static releaseTree(node) {
            for (const child of Array.from(node.childNodes ?? []))Patcher.releaseTree(child);
            Patcher.releaseScope(node);
        }
        static materialize(html, target, doc = target.ownerDocument ?? globalThis.document) {
            if (typeof doc?.createRange === "function") {
                const fragment = doc.createRange().createContextualFragment(html);
                target.appendChild(fragment);
            } else {
                target.innerHTML = html;
            }
            Hydrator.hydrate(target, doc);
            Patcher.runtime.dispatcher?.scanAndWire?.(target);
        }
        static ensureRegistry(doc) {
            if (Patcher.runtime.document !== doc) {
                Patcher.runtime.document = doc;
                Patcher.runtime.registry = new NodeRegistry().seedDocument(doc);
            }
            return Patcher.runtime.registry;
        }
        static applySignalPatches(patches) {
            const bridge = Patcher.runtime.signalBridge;
            if (bridge?.applyPatches) bridge.applyPatches(patches);
            else console.warn("Patcher: no signalBridge installed; JSON patches dropped", patches);
        }
        static route(result, targetEl, doc) {
            Patcher.applyRoute(result, targetEl, doc);
            if (result.type !== "frame") Patcher.markApplied();
        }
        static markApplied() {
            Patcher.frameSeq += 1;
            const stamp = typeof performance !== "undefined" ? performance.now() : Date.now();
            if (typeof globalThis !== "undefined") {
                globalThis.__primalFrames = {
                    seq: Patcher.frameSeq,
                    at: stamp
                };
            }
        }
        static applyRoute(result, targetEl, doc) {
            switch(result.type){
                case "frame":
                    if (!Patcher.runtime.frames) console.error("frame received but the frame runtime is not installed");
                    else for (const frame of result.frames)Patcher.runtime.frames.receive(frame, {
                        viewMount: targetEl
                    });
                    break;
                case "html":
                    Patcher.materialize(result.html, targetEl, doc);
                    break;
                case "html-morph":
                    {
                        const morphTarget = doc.querySelector(result.target);
                        if (morphTarget && typeof doc.createRange === "function") {
                            MorphDom.morph(morphTarget, doc.createRange().createContextualFragment(result.content), doc);
                        }
                        break;
                    }
                case "json-morph":
                    {
                        const morphTarget = doc.querySelector(result.morph.target);
                        if (morphTarget && typeof doc.createRange === "function") {
                            MorphDom.morph(morphTarget, doc.createRange().createContextualFragment(result.morph.content), doc);
                        }
                        break;
                    }
                case "json":
                    Patcher.applySignalPatches(result.patches);
                    break;
                default:
                    targetEl.textContent = result.text ?? "";
            }
        }
    }
    function frameMountTarget(element, target, doc) {
        if (!target) return element;
        return target === "parent" ? element.parentElement : doc.querySelector(target);
    }
    function resolveMountTarget(element, target, doc) {
        if (!target) {
            const container = doc.createElement("div");
            element.parentNode?.replaceChild?.(container, element);
            return container;
        }
        if (target === "parent") {
            const parent = element.parentElement ?? element.parent ?? null;
            if (!parent) throw new Error("mount target not found: parent");
            clearChildren(parent);
            return parent;
        }
        const found = doc.querySelector(target);
        if (!found) throw new Error(`mount target not found: ${target}`);
        clearChildren(found);
        return found;
    }
    function clearChildren(el) {
        while(el.firstChild)el.removeChild(el.firstChild);
    }
    const BaseElement = typeof HTMLElement === "undefined" ? class {
    } : HTMLElement;
    class IslandComponent extends BaseElement {
        connectedCallback() {
            Hydrator.hydrate(this);
            Patcher.runtime.dispatcher?.scanAndWire?.(this);
        }
        disconnectedCallback() {
            const doc = this.ownerDocument ?? globalThis.document;
            if (this._sheets?.length && doc?.adoptedStyleSheets) {
                doc.adoptedStyleSheets = doc.adoptedStyleSheets.filter((s)=>!this._sheets.includes(s));
            }
            this._sheets = [];
            Patcher.runtime.dispatcher?.removeListeners?.(this);
        }
    }
    function mountSetup(element, defaults = {}) {
        const api = element.getAttribute("api");
        if (!api) throw new Error(`${element.tagName?.toLowerCase() ?? "mount"}: missing api attribute`);
        let data = {};
        try {
            data = JSON.parse(element.getAttribute("data") || "{}");
        } catch (error) {
            console.error("mount: invalid JSON in data attribute", error);
        }
        const transport = element._transportOverride ?? Transport.create({
            transport: element.getAttribute("transport") ?? defaults.transport,
            protocol: element.getAttribute("protocol") ?? undefined,
            url: api
        });
        return {
            api,
            data,
            transport,
            target: element.getAttribute("target")
        };
    }
    class MountDataComponent extends BaseElement {
        async connectedCallback() {
            const doc = this.ownerDocument ?? globalThis.document;
            try {
                const binding = Patcher.runtime.onMountConnect?.(this, false);
                this._frameMount = binding;
                if (binding?.local) return;
                const { api, data, transport, target } = mountSetup(this);
                this._transport = transport;
                const method = this.getAttribute("method") || "POST";
                const result = await transport.send(binding ? binding.url(api) : api, method, data);
                if (binding) binding.receive(result);
                else {
                    const mount = result.type === "frame" ? frameMountTarget(this, target, doc) : resolveMountTarget(this, target, doc);
                    Patcher.route(result, mount, doc);
                }
            } catch (error) {
                console.error("mount-data:", error);
            }
        }
        disconnectedCallback() {
            this._frameMount?.close();
            this._transport?.disconnect?.();
        }
    }
    class MountStreamComponent extends BaseElement {
        connectedCallback() {
            const doc = this.ownerDocument ?? globalThis.document;
            try {
                const binding = Patcher.runtime.onMountConnect?.(this, true);
                this._frameMount = binding;
                if (binding?.local) return;
                let generation = 0;
                const open = ()=>{
                    const current = ++generation;
                    const { api, data, transport, target } = mountSetup(this, {
                        transport: "sse"
                    });
                    const targetEl = binding ? null : resolveMountTarget(this, target, doc);
                    if (binding?.managed) {
                        const handle = binding.managed({
                            api,
                            data,
                            transport
                        });
                        if (handle) {
                            this._transport = {
                                disconnect: ()=>handle.close()
                            };
                            return;
                        }
                    }
                    this._transport = transport;
                    if (!binding && this.isConnected === false && targetEl?.isConnected && !this._targetObserver) {
                        this._targetObserver = new MutationObserver(()=>{
                            if (!targetEl.isConnected) this.disconnectedCallback();
                        });
                        this._targetObserver.observe(doc, {
                            childList: true,
                            subtree: true
                        });
                    }
                    if (binding && transport.config) {
                        transport.config.nextUrl = (url)=>binding.url(url);
                        transport.config.onExpired = binding.expired;
                    }
                    const payload = binding && this.getAttribute("data") === null ? undefined : data;
                    transport.connect(binding ? binding.url(api) : api, payload, (result)=>{
                        if (current !== generation || transport !== this._transport) return;
                        if (binding) binding.receive(result);
                        else Patcher.route(result, targetEl, doc);
                    });
                };
                this._restartFrameStream = ()=>{
                    this._transport?.disconnect?.();
                    open();
                };
                open();
            } catch (error) {
                console.error("mount-stream:", error);
            }
        }
        disconnectedCallback() {
            this._frameMount?.close();
            this._restartFrameStream = null;
            this._targetObserver?.disconnect();
            this._targetObserver = null;
            this._transport?.disconnect?.();
            this._transport = null;
        }
    }
    function registerWebComponents() {
        if (typeof customElements === "undefined") return;
        if (!customElements.get("primal-island")) customElements.define("primal-island", IslandComponent);
        if (!customElements.get("mount-data")) customElements.define("mount-data", MountDataComponent);
        if (!customElements.get("mount-stream")) customElements.define("mount-stream", MountStreamComponent);
    }
    function registerWasmApp(runtime, opts = {}) {
        const doc = opts.document ?? globalThis.document;
        if (!doc) {
            console.error("registerWasmApp: no document available (non-browser environment)");
            return;
        }
        const bridge = runtime.bridge ?? runtime;
        const frames = globalThis.EweFrames?.installFrameRuntime(globalThis.FoundationWasmUiRuntime, opts);
        if (!frames) throw new Error("registerWasmApp requires foundation-ui-frames.js");
        const dispatcher = Patcher.runtime.dispatcher ?? new EventDispatcher(callbackDeliver(runtime.callbacks));
        dispatcher.deliver = callbackDeliver(runtime.callbacks);
        dispatcher.deliverSignal = signalDeliver(bridge);
        Patcher.runtime.dispatcher = dispatcher;
        const owned = new WeakSet();
        const onMount = (id, node)=>{
            if (!owned.has(node) && bridge.exports?.ewe_ui_dispose_mount) {
                owned.add(node);
                Patcher.registerUnmount(node, ()=>bridge.exports.ewe_ui_dispose_mount(id));
            }
        };
        const onEvent = (name, id, event, el)=>{
            const setter = parseCallbackId(el.getAttribute?.("primal:setter"));
            if (setter !== null) dispatcher.deliverSignal(setter, buildEventData(name, event, el));
        };
        runtime.dispatcher.setRawHandler(ProtocolHandler.frameHook.isFrame, (bytes)=>{
            if (bytes.byteLength > frames.maxFrameBytes) return frames.receive(bytes);
            return frames.receive(Uint8Array.from(bytes), {
                onMount,
                onEvent
            });
        });
        initEventRuntime(dispatcher, doc);
    }
    function frameStats() {
        return globalThis.__primalFrames ?? {
            seq: Patcher.frameSeq,
            at: null
        };
    }
    function createPrimal({ dispatcher, doc = globalThis.document } = {}) {
        return {
            mountData (api, data, targetNode, opts = {}) {
                const transport = Transport.create({
                    transport: opts.transport,
                    url: api
                });
                return transport.send(api, opts.method || "POST", data).then((result)=>Patcher.route(result, targetNode, doc));
            },
            mountStream (api, data, targetNode, opts = {}) {
                const transport = Transport.create({
                    transport: opts.transport ?? "sse",
                    url: api
                });
                transport.connect(api, data, (result)=>Patcher.route(result, targetNode, doc));
                return transport;
            },
            unmount (element) {
                element.disconnectedCallback?.();
                element.remove?.();
            },
            on: (selector, event, handler)=>doc.querySelector(selector)?.addEventListener(event, handler),
            onclick (selector, handler) {
                this.on(selector, "click", handler);
            },
            onchange (selector, handler) {
                this.on(selector, "change", handler);
            },
            off: (selector, event, handler)=>doc.querySelector(selector)?.removeEventListener(event, handler),
            scope: (element)=>Hydrator.createScope(element),
            dispatcher,
            Transport,
            ProtocolHandler,
            Patcher,
            Hydrator
        };
    }
    if (typeof window !== "undefined") {
        queueMicrotask(registerWebComponents);
        window.primal ??= createPrimal({});
    }
    globalThis.FoundationWasmUiRuntime = Object.freeze({
        RESERVED,
        NodeRegistry,
        MorphDom,
        moveBefore,
        buildEventData,
        parseCallbackId,
        EventDispatcher,
        CONVENIENCE_EVENTS,
        callbackDeliver,
        signalDeliver,
        resolveFunctionRef,
        resolveDelegateTarget,
        initEventRuntime,
        Transport,
        ProtocolHandler,
        Patcher,
        Hydrator,
        scopeCss,
        RequestQueue,
        SseParser,
        FetchEventSource,
        WSBatchQueue,
        WorkerBatchQueue,
        probeBatching,
        parseBatchEntry,
        reconnectDelay,
        resolveMountTarget,
        IslandComponent,
        MountDataComponent,
        MountStreamComponent,
        registerWebComponents,
        registerWasmApp,
        createPrimal,
        DomHeap,
        domAbi
    });
    const mod = {
        RESERVED: RESERVED,
        NodeRegistry: NodeRegistry,
        moveBefore: moveBefore,
        MorphDom: MorphDom,
        buildEventData: buildEventData,
        parseCallbackId: parseCallbackId,
        EventDispatcher: EventDispatcher,
        CONVENIENCE_EVENTS: CONVENIENCE_EVENTS,
        resolveFunctionRef: resolveFunctionRef,
        resolveDelegateTarget: resolveDelegateTarget,
        initEventRuntime: initEventRuntime,
        callbackDeliver: callbackDeliver,
        signalDeliver: signalDeliver,
        DomHeap: DomHeap,
        domAbi: domAbi,
        reconnectDelay: reconnectDelay,
        probeBatching: probeBatching,
        RequestQueue: RequestQueue,
        parseBatchEntry: parseBatchEntry,
        WSBatchQueue: WSBatchQueue,
        WorkerBatchQueue: WorkerBatchQueue,
        FetchTransport: FetchTransport,
        SseParser: SseParser,
        FetchEventSource: FetchEventSource,
        SSETransport: SSETransport,
        WebSocketTransport: WebSocketTransport,
        ChunkedTransport: ChunkedTransport,
        WorkerTransport: WorkerTransport,
        Transport: Transport,
        frameConnections: frameConnections,
        decodeEnvelopeFrame: decodeEnvelopeFrame,
        JsonHandler: JsonHandler,
        HtmlHandler: HtmlHandler,
        RawHandler: RawHandler,
        FrameHandler: FrameHandler,
        ProtocolHandler: ProtocolHandler,
        scopeCss: scopeCss,
        Hydrator: Hydrator,
        Patcher: Patcher,
        frameMountTarget: frameMountTarget,
        resolveMountTarget: resolveMountTarget,
        IslandComponent: IslandComponent,
        MountDataComponent: MountDataComponent,
        MountStreamComponent: MountStreamComponent,
        registerWebComponents: registerWebComponents,
        registerWasmApp: registerWasmApp,
        frameStats: frameStats,
        createPrimal: createPrimal
    };
    var SIZEOF_SHORT = 2;
    var SIZEOF_INT = 4;
    var FILE_IDENTIFIER_LENGTH = 4;
    var SIZE_PREFIX_LENGTH = 4;
    var int32 = new Int32Array(2);
    var float32 = new Float32Array(int32.buffer);
    var float64 = new Float64Array(int32.buffer);
    var isLittleEndian = new Uint16Array(new Uint8Array([
        1,
        0
    ]).buffer)[0] === 1;
    var Encoding;
    (function(Encoding2) {
        Encoding2[Encoding2["UTF8_BYTES"] = 1] = "UTF8_BYTES";
        Encoding2[Encoding2["UTF16_STRING"] = 2] = "UTF16_STRING";
    })(Encoding || (Encoding = {}));
    var ByteBuffer = class _ByteBuffer {
        constructor(bytes_){
            this.bytes_ = bytes_;
            this.position_ = 0;
            this.text_decoder_ = new TextDecoder;
        }
        static allocate(byte_size) {
            return new _ByteBuffer(new Uint8Array(byte_size));
        }
        clear() {
            this.position_ = 0;
        }
        bytes() {
            return this.bytes_;
        }
        position() {
            return this.position_;
        }
        setPosition(position) {
            this.position_ = position;
        }
        capacity() {
            return this.bytes_.length;
        }
        readInt8(offset) {
            return this.readUint8(offset) << 24 >> 24;
        }
        readUint8(offset) {
            return this.bytes_[offset];
        }
        readInt16(offset) {
            return this.readUint16(offset) << 16 >> 16;
        }
        readUint16(offset) {
            return this.bytes_[offset] | this.bytes_[offset + 1] << 8;
        }
        readInt32(offset) {
            return this.bytes_[offset] | this.bytes_[offset + 1] << 8 | this.bytes_[offset + 2] << 16 | this.bytes_[offset + 3] << 24;
        }
        readUint32(offset) {
            return this.readInt32(offset) >>> 0;
        }
        readInt64(offset) {
            return BigInt.asIntN(64, BigInt(this.readUint32(offset)) + (BigInt(this.readUint32(offset + 4)) << BigInt(32)));
        }
        readUint64(offset) {
            return BigInt.asUintN(64, BigInt(this.readUint32(offset)) + (BigInt(this.readUint32(offset + 4)) << BigInt(32)));
        }
        readFloat32(offset) {
            int32[0] = this.readInt32(offset);
            return float32[0];
        }
        readFloat64(offset) {
            int32[isLittleEndian ? 0 : 1] = this.readInt32(offset);
            int32[isLittleEndian ? 1 : 0] = this.readInt32(offset + 4);
            return float64[0];
        }
        writeInt8(offset, value) {
            this.bytes_[offset] = value;
        }
        writeUint8(offset, value) {
            this.bytes_[offset] = value;
        }
        writeInt16(offset, value) {
            this.bytes_[offset] = value;
            this.bytes_[offset + 1] = value >> 8;
        }
        writeUint16(offset, value) {
            this.bytes_[offset] = value;
            this.bytes_[offset + 1] = value >> 8;
        }
        writeInt32(offset, value) {
            this.bytes_[offset] = value;
            this.bytes_[offset + 1] = value >> 8;
            this.bytes_[offset + 2] = value >> 16;
            this.bytes_[offset + 3] = value >> 24;
        }
        writeUint32(offset, value) {
            this.bytes_[offset] = value;
            this.bytes_[offset + 1] = value >> 8;
            this.bytes_[offset + 2] = value >> 16;
            this.bytes_[offset + 3] = value >> 24;
        }
        writeInt64(offset, value) {
            this.writeInt32(offset, Number(BigInt.asIntN(32, value)));
            this.writeInt32(offset + 4, Number(BigInt.asIntN(32, value >> BigInt(32))));
        }
        writeUint64(offset, value) {
            this.writeUint32(offset, Number(BigInt.asUintN(32, value)));
            this.writeUint32(offset + 4, Number(BigInt.asUintN(32, value >> BigInt(32))));
        }
        writeFloat32(offset, value) {
            float32[0] = value;
            this.writeInt32(offset, int32[0]);
        }
        writeFloat64(offset, value) {
            float64[0] = value;
            this.writeInt32(offset, int32[isLittleEndian ? 0 : 1]);
            this.writeInt32(offset + 4, int32[isLittleEndian ? 1 : 0]);
        }
        getBufferIdentifier() {
            if (this.bytes_.length < this.position_ + SIZEOF_INT + FILE_IDENTIFIER_LENGTH) {
                throw new Error("FlatBuffers: ByteBuffer is too short to contain an identifier.");
            }
            let result = "";
            for(let i = 0; i < FILE_IDENTIFIER_LENGTH; i++){
                result += String.fromCharCode(this.readInt8(this.position_ + SIZEOF_INT + i));
            }
            return result;
        }
        __offset(bb_pos, vtable_offset) {
            const vtable = bb_pos - this.readInt32(bb_pos);
            return vtable_offset < this.readInt16(vtable) ? this.readInt16(vtable + vtable_offset) : 0;
        }
        __union(t, offset) {
            t.bb_pos = offset + this.readInt32(offset);
            t.bb = this;
            return t;
        }
        __string(offset, opt_encoding) {
            offset += this.readInt32(offset);
            const length = this.readInt32(offset);
            offset += SIZEOF_INT;
            const utf8bytes = this.bytes_.subarray(offset, offset + length);
            if (opt_encoding === Encoding.UTF8_BYTES) return utf8bytes;
            else return this.text_decoder_.decode(utf8bytes);
        }
        __union_with_string(o, offset) {
            if (typeof o === "string") {
                return this.__string(offset);
            }
            return this.__union(o, offset);
        }
        __indirect(offset) {
            return offset + this.readInt32(offset);
        }
        __vector(offset) {
            return offset + this.readInt32(offset) + SIZEOF_INT;
        }
        __vector_len(offset) {
            return this.readInt32(offset + this.readInt32(offset));
        }
        __has_identifier(ident) {
            if (ident.length != FILE_IDENTIFIER_LENGTH) {
                throw new Error("FlatBuffers: file identifier must be length " + FILE_IDENTIFIER_LENGTH);
            }
            for(let i = 0; i < FILE_IDENTIFIER_LENGTH; i++){
                if (ident.charCodeAt(i) != this.readInt8(this.position() + SIZEOF_INT + i)) {
                    return false;
                }
            }
            return true;
        }
        createScalarList(listAccessor, listLength) {
            const ret = [];
            for(let i = 0; i < listLength; ++i){
                const val = listAccessor(i);
                if (val !== null) {
                    ret.push(val);
                }
            }
            return ret;
        }
        createObjList(listAccessor, listLength) {
            const ret = [];
            for(let i = 0; i < listLength; ++i){
                const val = listAccessor(i);
                if (val !== null) {
                    ret.push(val.unpack());
                }
            }
            return ret;
        }
    };
    var Builder = class _Builder {
        constructor(opt_initial_size){
            this.minalign = 1;
            this.vtable = null;
            this.vtable_in_use = 0;
            this.isNested = false;
            this.object_start = 0;
            this.vtables = [];
            this.vector_num_elems = 0;
            this.force_defaults = false;
            this.string_maps = null;
            this.text_encoder = new TextEncoder;
            let initial_size;
            if (!opt_initial_size) {
                initial_size = 1024;
            } else {
                initial_size = opt_initial_size;
            }
            this.bb = ByteBuffer.allocate(initial_size);
            this.space = initial_size;
        }
        clear() {
            this.bb.clear();
            this.space = this.bb.capacity();
            this.minalign = 1;
            this.vtable = null;
            this.vtable_in_use = 0;
            this.isNested = false;
            this.object_start = 0;
            this.vtables = [];
            this.vector_num_elems = 0;
            this.force_defaults = false;
            this.string_maps = null;
        }
        forceDefaults(forceDefaults) {
            this.force_defaults = forceDefaults;
        }
        dataBuffer() {
            return this.bb;
        }
        asUint8Array() {
            return this.bb.bytes().subarray(this.bb.position(), this.bb.position() + this.offset());
        }
        prep(size, additional_bytes) {
            if (size > this.minalign) {
                this.minalign = size;
            }
            const align_size = ~(this.bb.capacity() - this.space + additional_bytes) + 1 & size - 1;
            while(this.space < align_size + size + additional_bytes){
                const old_buf_size = this.bb.capacity();
                this.bb = _Builder.growByteBuffer(this.bb);
                this.space += this.bb.capacity() - old_buf_size;
            }
            this.pad(align_size);
        }
        pad(byte_size) {
            for(let i = 0; i < byte_size; i++){
                this.bb.writeInt8(--this.space, 0);
            }
        }
        writeInt8(value) {
            this.bb.writeInt8(this.space -= 1, value);
        }
        writeInt16(value) {
            this.bb.writeInt16(this.space -= 2, value);
        }
        writeInt32(value) {
            this.bb.writeInt32(this.space -= 4, value);
        }
        writeInt64(value) {
            this.bb.writeInt64(this.space -= 8, value);
        }
        writeFloat32(value) {
            this.bb.writeFloat32(this.space -= 4, value);
        }
        writeFloat64(value) {
            this.bb.writeFloat64(this.space -= 8, value);
        }
        addInt8(value) {
            this.prep(1, 0);
            this.writeInt8(value);
        }
        addInt16(value) {
            this.prep(2, 0);
            this.writeInt16(value);
        }
        addInt32(value) {
            this.prep(4, 0);
            this.writeInt32(value);
        }
        addInt64(value) {
            this.prep(8, 0);
            this.writeInt64(value);
        }
        addFloat32(value) {
            this.prep(4, 0);
            this.writeFloat32(value);
        }
        addFloat64(value) {
            this.prep(8, 0);
            this.writeFloat64(value);
        }
        addFieldInt8(voffset, value, defaultValue) {
            if (this.force_defaults || value != defaultValue) {
                this.addInt8(value);
                this.slot(voffset);
            }
        }
        addFieldInt16(voffset, value, defaultValue) {
            if (this.force_defaults || value != defaultValue) {
                this.addInt16(value);
                this.slot(voffset);
            }
        }
        addFieldInt32(voffset, value, defaultValue) {
            if (this.force_defaults || value != defaultValue) {
                this.addInt32(value);
                this.slot(voffset);
            }
        }
        addFieldInt64(voffset, value, defaultValue) {
            if (this.force_defaults || value !== defaultValue) {
                this.addInt64(value);
                this.slot(voffset);
            }
        }
        addFieldFloat32(voffset, value, defaultValue) {
            if (this.force_defaults || value != defaultValue) {
                this.addFloat32(value);
                this.slot(voffset);
            }
        }
        addFieldFloat64(voffset, value, defaultValue) {
            if (this.force_defaults || value != defaultValue) {
                this.addFloat64(value);
                this.slot(voffset);
            }
        }
        addFieldOffset(voffset, value, defaultValue) {
            if (this.force_defaults || value != defaultValue) {
                this.addOffset(value);
                this.slot(voffset);
            }
        }
        addFieldStruct(voffset, value, defaultValue) {
            if (value != defaultValue) {
                this.nested(value);
                this.slot(voffset);
            }
        }
        nested(obj) {
            if (obj != this.offset()) {
                throw new TypeError("FlatBuffers: struct must be serialized inline.");
            }
        }
        notNested() {
            if (this.isNested) {
                throw new TypeError("FlatBuffers: object serialization must not be nested.");
            }
        }
        slot(voffset) {
            if (this.vtable !== null) this.vtable[voffset] = this.offset();
        }
        offset() {
            return this.bb.capacity() - this.space;
        }
        static growByteBuffer(bb) {
            const old_buf_size = bb.capacity();
            if (old_buf_size & 3221225472) {
                throw new Error("FlatBuffers: cannot grow buffer beyond 2 gigabytes.");
            }
            const new_buf_size = old_buf_size << 1;
            const nbb = ByteBuffer.allocate(new_buf_size);
            nbb.setPosition(new_buf_size - old_buf_size);
            nbb.bytes().set(bb.bytes(), new_buf_size - old_buf_size);
            return nbb;
        }
        addOffset(offset) {
            this.prep(SIZEOF_INT, 0);
            this.writeInt32(this.offset() - offset + SIZEOF_INT);
        }
        startObject(numfields) {
            this.notNested();
            if (this.vtable == null) {
                this.vtable = [];
            }
            this.vtable_in_use = numfields;
            for(let i = 0; i < numfields; i++){
                this.vtable[i] = 0;
            }
            this.isNested = true;
            this.object_start = this.offset();
        }
        endObject() {
            if (this.vtable == null || !this.isNested) {
                throw new Error("FlatBuffers: endObject called without startObject");
            }
            this.addInt32(0);
            const vtableloc = this.offset();
            let i = this.vtable_in_use - 1;
            for(; i >= 0 && this.vtable[i] == 0; i--){}
            const trimmed_size = i + 1;
            for(; i >= 0; i--){
                this.addInt16(this.vtable[i] != 0 ? vtableloc - this.vtable[i] : 0);
            }
            this.addInt16(vtableloc - this.object_start);
            const len = (trimmed_size + 2) * SIZEOF_SHORT;
            this.addInt16(len);
            let existing_vtable = 0;
            const vt1 = this.space;
            outer_loop: for(i = 0; i < this.vtables.length; i++){
                const vt2 = this.bb.capacity() - this.vtables[i];
                if (len == this.bb.readInt16(vt2)) {
                    for(let j = SIZEOF_SHORT; j < len; j += SIZEOF_SHORT){
                        if (this.bb.readInt16(vt1 + j) != this.bb.readInt16(vt2 + j)) {
                            continue outer_loop;
                        }
                    }
                    existing_vtable = this.vtables[i];
                    break;
                }
            }
            if (existing_vtable) {
                this.space = this.bb.capacity() - vtableloc;
                this.bb.writeInt32(this.space, existing_vtable - vtableloc);
            } else {
                this.vtables.push(this.offset());
                this.bb.writeInt32(this.bb.capacity() - vtableloc, this.offset() - vtableloc);
            }
            this.isNested = false;
            return vtableloc;
        }
        finish(root_table, opt_file_identifier, opt_size_prefix) {
            const size_prefix = opt_size_prefix ? SIZE_PREFIX_LENGTH : 0;
            if (opt_file_identifier) {
                const file_identifier = opt_file_identifier;
                this.prep(this.minalign, SIZEOF_INT + FILE_IDENTIFIER_LENGTH + size_prefix);
                if (file_identifier.length != FILE_IDENTIFIER_LENGTH) {
                    throw new TypeError("FlatBuffers: file identifier must be length " + FILE_IDENTIFIER_LENGTH);
                }
                for(let i = FILE_IDENTIFIER_LENGTH - 1; i >= 0; i--){
                    this.writeInt8(file_identifier.charCodeAt(i));
                }
            }
            this.prep(this.minalign, SIZEOF_INT + size_prefix);
            this.addOffset(root_table);
            if (size_prefix) {
                this.addInt32(this.bb.capacity() - this.space);
            }
            this.bb.setPosition(this.space);
        }
        finishSizePrefixed(root_table, opt_file_identifier) {
            this.finish(root_table, opt_file_identifier, true);
        }
        requiredField(table, field) {
            const table_start = this.bb.capacity() - table;
            const vtable_start = table_start - this.bb.readInt32(table_start);
            const ok = field < this.bb.readInt16(vtable_start) && this.bb.readInt16(vtable_start + field) != 0;
            if (!ok) {
                throw new TypeError("FlatBuffers: field " + field + " must be set");
            }
        }
        startVector(elem_size, num_elems, alignment) {
            this.notNested();
            this.vector_num_elems = num_elems;
            this.prep(SIZEOF_INT, elem_size * num_elems);
            this.prep(alignment, elem_size * num_elems);
        }
        endVector() {
            this.writeInt32(this.vector_num_elems);
            return this.offset();
        }
        createSharedString(s) {
            if (!s) {
                return 0;
            }
            if (!this.string_maps) {
                this.string_maps = new Map;
            }
            if (this.string_maps.has(s)) {
                return this.string_maps.get(s);
            }
            const offset = this.createString(s);
            this.string_maps.set(s, offset);
            return offset;
        }
        createString(s) {
            if (s === null || s === void 0) {
                return 0;
            }
            let utf8;
            if (s instanceof Uint8Array) {
                utf8 = s;
            } else {
                utf8 = this.text_encoder.encode(s);
            }
            this.addInt8(0);
            this.startVector(1, utf8.length, 1);
            this.bb.setPosition(this.space -= utf8.length);
            this.bb.bytes().set(utf8, this.space);
            return this.endVector();
        }
        createByteVector(v) {
            if (v === null || v === void 0) {
                return 0;
            }
            this.startVector(1, v.length, 1);
            this.bb.setPosition(this.space -= v.length);
            this.bb.bytes().set(v, this.space);
            return this.endVector();
        }
        createObjectOffset(obj) {
            if (obj === null) {
                return 0;
            }
            if (typeof obj === "string") {
                return this.createString(obj);
            } else {
                return obj.pack(this);
            }
        }
        createObjectOffsetList(list) {
            const ret = [];
            for(let i = 0; i < list.length; ++i){
                const val = list[i];
                if (val !== null) {
                    ret.push(this.createObjectOffset(val));
                } else {
                    throw new TypeError("FlatBuffers: Argument for createObjectOffsetList cannot contain null.");
                }
            }
            return ret;
        }
        createStructOffsetList(list, startFunc) {
            startFunc(this, list.length);
            this.createObjectOffsetList(list.slice().reverse());
            return this.endVector();
        }
    };
    var ErrorCode = ((ErrorCode2)=>{
        ErrorCode2[ErrorCode2["Unknown"] = 0] = "Unknown";
        ErrorCode2[ErrorCode2["BadRequest"] = 1] = "BadRequest";
        ErrorCode2[ErrorCode2["UnknownAction"] = 2] = "UnknownAction";
        ErrorCode2[ErrorCode2["UnavailableOrigin"] = 3] = "UnavailableOrigin";
        ErrorCode2[ErrorCode2["UnsupportedAction"] = 4] = "UnsupportedAction";
        ErrorCode2[ErrorCode2["StateRejected"] = 5] = "StateRejected";
        ErrorCode2[ErrorCode2["StateExpired"] = 6] = "StateExpired";
        ErrorCode2[ErrorCode2["RateLimited"] = 7] = "RateLimited";
        ErrorCode2[ErrorCode2["ActionFailed"] = 8] = "ActionFailed";
        ErrorCode2[ErrorCode2["UpdateRequired"] = 9] = "UpdateRequired";
        ErrorCode2[ErrorCode2["UnsupportedPlatformOp"] = 10] = "UnsupportedPlatformOp";
        ErrorCode2[ErrorCode2["InvalidInput"] = 11] = "InvalidInput";
        return ErrorCode2;
    })(ErrorCode || {});
    var ActionError = class _ActionError {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsActionError(bb, obj) {
            return (obj || new _ActionError).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsActionError(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _ActionError).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        code() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        message(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        rowIndex() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        static startActionError(builder) {
            builder.startObject(3);
        }
        static addCode(builder, code) {
            builder.addFieldInt16(0, code, 0);
        }
        static addMessage(builder, messageOffset) {
            builder.addFieldOffset(1, messageOffset, 0);
        }
        static addRowIndex(builder, rowIndex) {
            builder.addFieldInt32(2, rowIndex, 0);
        }
        static endActionError(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createActionError(builder, code, messageOffset, rowIndex) {
            _ActionError.startActionError(builder);
            _ActionError.addCode(builder, code);
            _ActionError.addMessage(builder, messageOffset);
            _ActionError.addRowIndex(builder, rowIndex);
            return _ActionError.endActionError(builder);
        }
        unpack() {
            return new ActionErrorT(this.code(), this.message(), this.rowIndex());
        }
        unpackTo(_o) {
            _o.code = this.code();
            _o.message = this.message();
            _o.rowIndex = this.rowIndex();
        }
    };
    var ActionErrorT = class {
        constructor(code = 0, message = null, rowIndex = 0){
            this.code = code;
            this.message = message;
            this.rowIndex = rowIndex;
        }
        code;
        message;
        rowIndex;
        pack(builder) {
            const message = this.message !== null ? builder.createString(this.message) : 0;
            return ActionError.createActionError(builder, this.code, message, this.rowIndex);
        }
    };
    var ActionInput = class _ActionInput {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsActionInput(bb, obj) {
            return (obj || new _ActionInput).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsActionInput(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _ActionInput).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        eventName(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        value(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        checked() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? !!this.bb.readInt8(this.bb_pos + offset) : false;
        }
        keyCode() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        modifiers() {
            const offset = this.bb.__offset(this.bb_pos, 12);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        parentNode() {
            const offset = this.bb.__offset(this.bb_pos, 14);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        json(index) {
            const offset = this.bb.__offset(this.bb_pos, 16);
            return offset ? this.bb.readUint8(this.bb.__vector(this.bb_pos + offset) + index) : 0;
        }
        jsonLength() {
            const offset = this.bb.__offset(this.bb_pos, 16);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        jsonArray() {
            const offset = this.bb.__offset(this.bb_pos, 16);
            return offset ? new Uint8Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        static startActionInput(builder) {
            builder.startObject(7);
        }
        static addEventName(builder, eventNameOffset) {
            builder.addFieldOffset(0, eventNameOffset, 0);
        }
        static addValue(builder, valueOffset) {
            builder.addFieldOffset(1, valueOffset, 0);
        }
        static addChecked(builder, checked) {
            builder.addFieldInt8(2, +checked, 0);
        }
        static addKeyCode(builder, keyCode) {
            builder.addFieldInt32(3, keyCode, 0);
        }
        static addModifiers(builder, modifiers) {
            builder.addFieldInt8(4, modifiers, 0);
        }
        static addParentNode(builder, parentNode) {
            builder.addFieldInt32(5, parentNode, 0);
        }
        static addJson(builder, jsonOffset) {
            builder.addFieldOffset(6, jsonOffset, 0);
        }
        static createJsonVector(builder, data) {
            builder.startVector(1, data.length, 1);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt8(data[i]);
            }
            return builder.endVector();
        }
        static startJsonVector(builder, numElems) {
            builder.startVector(1, numElems, 1);
        }
        static endActionInput(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createActionInput(builder, eventNameOffset, valueOffset, checked, keyCode, modifiers, parentNode, jsonOffset) {
            _ActionInput.startActionInput(builder);
            _ActionInput.addEventName(builder, eventNameOffset);
            _ActionInput.addValue(builder, valueOffset);
            _ActionInput.addChecked(builder, checked);
            _ActionInput.addKeyCode(builder, keyCode);
            _ActionInput.addModifiers(builder, modifiers);
            _ActionInput.addParentNode(builder, parentNode);
            _ActionInput.addJson(builder, jsonOffset);
            return _ActionInput.endActionInput(builder);
        }
        unpack() {
            return new ActionInputT(this.eventName(), this.value(), this.checked(), this.keyCode(), this.modifiers(), this.parentNode(), this.jsonArray() === null ? null : Array.from(this.jsonArray()));
        }
        unpackTo(_o) {
            _o.eventName = this.eventName();
            _o.value = this.value();
            _o.checked = this.checked();
            _o.keyCode = this.keyCode();
            _o.modifiers = this.modifiers();
            _o.parentNode = this.parentNode();
            _o.json = this.jsonArray() === null ? null : Array.from(this.jsonArray());
        }
    };
    var ActionInputT = class {
        constructor(eventName = null, value = null, checked = false, keyCode = 0, modifiers = 0, parentNode = 0, json = null){
            this.eventName = eventName;
            this.value = value;
            this.checked = checked;
            this.keyCode = keyCode;
            this.modifiers = modifiers;
            this.parentNode = parentNode;
            this.json = json;
        }
        eventName;
        value;
        checked;
        keyCode;
        modifiers;
        parentNode;
        json;
        pack(builder) {
            const eventName = this.eventName !== null ? builder.createString(this.eventName) : 0;
            const value = this.value !== null ? builder.createString(this.value) : 0;
            const json = this.json === null ? 0 : ActionInput.createJsonVector(builder, this.json);
            return ActionInput.createActionInput(builder, eventName, value, this.checked, this.keyCode, this.modifiers, this.parentNode, json);
        }
    };
    var ActionRequest = class _ActionRequest {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsActionRequest(bb, obj) {
            return (obj || new _ActionRequest).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsActionRequest(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _ActionRequest).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        action(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        input(obj) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? (obj || new ActionInput).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        state(index) {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.readUint8(this.bb.__vector(this.bb_pos + offset) + index) : 0;
        }
        stateLength() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        stateArray() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? new Uint8Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        static startActionRequest(builder) {
            builder.startObject(4);
        }
        static addAction(builder, actionOffset) {
            builder.addFieldOffset(0, actionOffset, 0);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(1, nodeId, 0);
        }
        static addInput(builder, inputOffset) {
            builder.addFieldOffset(2, inputOffset, 0);
        }
        static addState(builder, stateOffset) {
            builder.addFieldOffset(3, stateOffset, 0);
        }
        static createStateVector(builder, data) {
            builder.startVector(1, data.length, 1);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt8(data[i]);
            }
            return builder.endVector();
        }
        static startStateVector(builder, numElems) {
            builder.startVector(1, numElems, 1);
        }
        static endActionRequest(builder) {
            const offset = builder.endObject();
            return offset;
        }
        unpack() {
            return new ActionRequestT(this.action(), this.nodeId(), this.input() !== null ? this.input().unpack() : null, this.bb.createScalarList(this.state.bind(this), this.stateLength()));
        }
        unpackTo(_o) {
            _o.action = this.action();
            _o.nodeId = this.nodeId();
            _o.input = this.input() !== null ? this.input().unpack() : null;
            _o.state = this.bb.createScalarList(this.state.bind(this), this.stateLength());
        }
    };
    var ActionRequestT = class {
        constructor(action = null, nodeId = 0, input = null, state = []){
            this.action = action;
            this.nodeId = nodeId;
            this.input = input;
            this.state = state;
        }
        action;
        nodeId;
        input;
        state;
        pack(builder) {
            const action = this.action !== null ? builder.createString(this.action) : 0;
            const input = this.input !== null ? this.input.pack(builder) : 0;
            const state = ActionRequest.createStateVector(builder, this.state);
            ActionRequest.startActionRequest(builder);
            ActionRequest.addAction(builder, action);
            ActionRequest.addNodeId(builder, this.nodeId);
            ActionRequest.addInput(builder, input);
            ActionRequest.addState(builder, state);
            return ActionRequest.endActionRequest(builder);
        }
    };
    var Alignment = ((Alignment2)=>{
        Alignment2[Alignment2["Start"] = 0] = "Start";
        Alignment2[Alignment2["Center"] = 1] = "Center";
        Alignment2[Alignment2["End"] = 2] = "End";
        Alignment2[Alignment2["Stretch"] = 3] = "Stretch";
        return Alignment2;
    })(Alignment || {});
    var AlignmentValue = class _AlignmentValue {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsAlignmentValue(bb, obj) {
            return (obj || new _AlignmentValue).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsAlignmentValue(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _AlignmentValue).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        v() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        static startAlignmentValue(builder) {
            builder.startObject(1);
        }
        static addV(builder, v) {
            builder.addFieldInt8(0, v, 0);
        }
        static endAlignmentValue(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createAlignmentValue(builder, v) {
            _AlignmentValue.startAlignmentValue(builder);
            _AlignmentValue.addV(builder, v);
            return _AlignmentValue.endAlignmentValue(builder);
        }
        unpack() {
            return new AlignmentValueT(this.v());
        }
        unpackTo(_o) {
            _o.v = this.v();
        }
    };
    var AlignmentValueT = class {
        constructor(v = 0){
            this.v = v;
        }
        v;
        pack(builder) {
            return AlignmentValue.createAlignmentValue(builder, this.v);
        }
    };
    var AppendChild = class _AppendChild {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsAppendChild(bb, obj) {
            return (obj || new _AppendChild).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsAppendChild(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _AppendChild).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        parentId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        childId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        static startAppendChild(builder) {
            builder.startObject(2);
        }
        static addParentId(builder, parentId) {
            builder.addFieldInt32(0, parentId, 0);
        }
        static addChildId(builder, childId) {
            builder.addFieldInt32(1, childId, 0);
        }
        static endAppendChild(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createAppendChild(builder, parentId, childId) {
            _AppendChild.startAppendChild(builder);
            _AppendChild.addParentId(builder, parentId);
            _AppendChild.addChildId(builder, childId);
            return _AppendChild.endAppendChild(builder);
        }
        unpack() {
            return new AppendChildT(this.parentId(), this.childId());
        }
        unpackTo(_o) {
            _o.parentId = this.parentId();
            _o.childId = this.childId();
        }
    };
    var AppendChildT = class {
        constructor(parentId = 0, childId = 0){
            this.parentId = parentId;
            this.childId = childId;
        }
        parentId;
        childId;
        pack(builder) {
            return AppendChild.createAppendChild(builder, this.parentId, this.childId);
        }
    };
    var ArrowIpcBlob = class _ArrowIpcBlob {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsArrowIpcBlob(bb, obj) {
            return (obj || new _ArrowIpcBlob).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsArrowIpcBlob(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _ArrowIpcBlob).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        targetNode() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        bytes(index) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint8(this.bb.__vector(this.bb_pos + offset) + index) : 0;
        }
        bytesLength() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        bytesArray() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? new Uint8Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        static startArrowIpcBlob(builder) {
            builder.startObject(2);
        }
        static addTargetNode(builder, targetNode) {
            builder.addFieldInt32(0, targetNode, 0);
        }
        static addBytes(builder, bytesOffset) {
            builder.addFieldOffset(1, bytesOffset, 0);
        }
        static createBytesVector(builder, data) {
            builder.startVector(1, data.length, 1);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt8(data[i]);
            }
            return builder.endVector();
        }
        static startBytesVector(builder, numElems) {
            builder.startVector(1, numElems, 1);
        }
        static endArrowIpcBlob(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createArrowIpcBlob(builder, targetNode, bytesOffset) {
            _ArrowIpcBlob.startArrowIpcBlob(builder);
            _ArrowIpcBlob.addTargetNode(builder, targetNode);
            _ArrowIpcBlob.addBytes(builder, bytesOffset);
            return _ArrowIpcBlob.endArrowIpcBlob(builder);
        }
        unpack() {
            return new ArrowIpcBlobT(this.targetNode(), this.bb.createScalarList(this.bytes.bind(this), this.bytesLength()));
        }
        unpackTo(_o) {
            _o.targetNode = this.targetNode();
            _o.bytes = this.bb.createScalarList(this.bytes.bind(this), this.bytesLength());
        }
    };
    var ArrowIpcBlobT = class {
        constructor(targetNode = 0, bytes = []){
            this.targetNode = targetNode;
            this.bytes = bytes;
        }
        targetNode;
        bytes;
        pack(builder) {
            const bytes = ArrowIpcBlob.createBytesVector(builder, this.bytes);
            return ArrowIpcBlob.createArrowIpcBlob(builder, this.targetNode, bytes);
        }
    };
    var AttachState = class _AttachState {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsAttachState(bb, obj) {
            return (obj || new _AttachState).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsAttachState(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _AttachState).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        rootNode() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        state(index) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint8(this.bb.__vector(this.bb_pos + offset) + index) : 0;
        }
        stateLength() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        stateArray() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? new Uint8Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        static startAttachState(builder) {
            builder.startObject(2);
        }
        static addRootNode(builder, rootNode) {
            builder.addFieldInt32(0, rootNode, 0);
        }
        static addState(builder, stateOffset) {
            builder.addFieldOffset(1, stateOffset, 0);
        }
        static createStateVector(builder, data) {
            builder.startVector(1, data.length, 1);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt8(data[i]);
            }
            return builder.endVector();
        }
        static startStateVector(builder, numElems) {
            builder.startVector(1, numElems, 1);
        }
        static endAttachState(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createAttachState(builder, rootNode, stateOffset) {
            _AttachState.startAttachState(builder);
            _AttachState.addRootNode(builder, rootNode);
            _AttachState.addState(builder, stateOffset);
            return _AttachState.endAttachState(builder);
        }
        unpack() {
            return new AttachStateT(this.rootNode(), this.bb.createScalarList(this.state.bind(this), this.stateLength()));
        }
        unpackTo(_o) {
            _o.rootNode = this.rootNode();
            _o.state = this.bb.createScalarList(this.state.bind(this), this.stateLength());
        }
    };
    var AttachStateT = class {
        constructor(rootNode = 0, state = []){
            this.rootNode = rootNode;
            this.state = state;
        }
        rootNode;
        state;
        pack(builder) {
            const state = AttachState.createStateVector(builder, this.state);
            return AttachState.createAttachState(builder, this.rootNode, state);
        }
    };
    var Axis = ((Axis2)=>{
        Axis2[Axis2["Vertical"] = 0] = "Vertical";
        Axis2[Axis2["Horizontal"] = 1] = "Horizontal";
        return Axis2;
    })(Axis || {});
    var AxisValue = class _AxisValue {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsAxisValue(bb, obj) {
            return (obj || new _AxisValue).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsAxisValue(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _AxisValue).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        v() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        static startAxisValue(builder) {
            builder.startObject(1);
        }
        static addV(builder, v) {
            builder.addFieldInt8(0, v, 0);
        }
        static endAxisValue(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createAxisValue(builder, v) {
            _AxisValue.startAxisValue(builder);
            _AxisValue.addV(builder, v);
            return _AxisValue.endAxisValue(builder);
        }
        unpack() {
            return new AxisValueT(this.v());
        }
        unpackTo(_o) {
            _o.v = this.v();
        }
    };
    var AxisValueT = class {
        constructor(v = 0){
            this.v = v;
        }
        v;
        pack(builder) {
            return AxisValue.createAxisValue(builder, this.v);
        }
    };
    var BoolValue = class _BoolValue {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsBoolValue(bb, obj) {
            return (obj || new _BoolValue).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsBoolValue(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _BoolValue).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        v() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? !!this.bb.readInt8(this.bb_pos + offset) : false;
        }
        static startBoolValue(builder) {
            builder.startObject(1);
        }
        static addV(builder, v) {
            builder.addFieldInt8(0, +v, 0);
        }
        static endBoolValue(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createBoolValue(builder, v) {
            _BoolValue.startBoolValue(builder);
            _BoolValue.addV(builder, v);
            return _BoolValue.endBoolValue(builder);
        }
        unpack() {
            return new BoolValueT(this.v());
        }
        unpackTo(_o) {
            _o.v = this.v();
        }
    };
    var BoolValueT = class {
        constructor(v = false){
            this.v = v;
        }
        v;
        pack(builder) {
            return BoolValue.createBoolValue(builder, this.v);
        }
    };
    var ColorToken = class _ColorToken {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsColorToken(bb, obj) {
            return (obj || new _ColorToken).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsColorToken(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _ColorToken).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        id() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        static startColorToken(builder) {
            builder.startObject(1);
        }
        static addId(builder, id) {
            builder.addFieldInt32(0, id, 0);
        }
        static endColorToken(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createColorToken(builder, id) {
            _ColorToken.startColorToken(builder);
            _ColorToken.addId(builder, id);
            return _ColorToken.endColorToken(builder);
        }
        unpack() {
            return new ColorTokenT(this.id());
        }
        unpackTo(_o) {
            _o.id = this.id();
        }
    };
    var ColorTokenT = class {
        constructor(id = 0){
            this.id = id;
        }
        id;
        pack(builder) {
            return ColorToken.createColorToken(builder, this.id);
        }
    };
    var FontToken = class _FontToken {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsFontToken(bb, obj) {
            return (obj || new _FontToken).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsFontToken(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _FontToken).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        id() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        static startFontToken(builder) {
            builder.startObject(1);
        }
        static addId(builder, id) {
            builder.addFieldInt32(0, id, 0);
        }
        static endFontToken(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createFontToken(builder, id) {
            _FontToken.startFontToken(builder);
            _FontToken.addId(builder, id);
            return _FontToken.endFontToken(builder);
        }
        unpack() {
            return new FontTokenT(this.id());
        }
        unpackTo(_o) {
            _o.id = this.id();
        }
    };
    var FontTokenT = class {
        constructor(id = 0){
            this.id = id;
        }
        id;
        pack(builder) {
            return FontToken.createFontToken(builder, this.id);
        }
    };
    var LengthUnit = ((LengthUnit2)=>{
        LengthUnit2[LengthUnit2["Point"] = 0] = "Point";
        LengthUnit2[LengthUnit2["Percent"] = 1] = "Percent";
        LengthUnit2[LengthUnit2["Auto"] = 2] = "Auto";
        return LengthUnit2;
    })(LengthUnit || {});
    var Length = class {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        value() {
            return this.bb.readFloat32(this.bb_pos);
        }
        unit() {
            return this.bb.readUint8(this.bb_pos + 4);
        }
        static sizeOf() {
            return 8;
        }
        static createLength(builder, value, unit) {
            builder.prep(4, 8);
            builder.pad(3);
            builder.writeInt8(unit);
            builder.writeFloat32(value);
            return builder.offset();
        }
        unpack() {
            return new LengthT(this.value(), this.unit());
        }
        unpackTo(_o) {
            _o.value = this.value();
            _o.unit = this.unit();
        }
    };
    var LengthT = class {
        constructor(value = 0, unit = 0){
            this.value = value;
            this.unit = unit;
        }
        value;
        unit;
        pack(builder) {
            return Length.createLength(builder, this.value, this.unit);
        }
    };
    var LengthValue = class _LengthValue {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsLengthValue(bb, obj) {
            return (obj || new _LengthValue).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsLengthValue(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _LengthValue).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        v(obj) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? (obj || new Length).__init(this.bb_pos + offset, this.bb) : null;
        }
        static startLengthValue(builder) {
            builder.startObject(1);
        }
        static addV(builder, vOffset) {
            builder.addFieldStruct(0, vOffset, 0);
        }
        static endLengthValue(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createLengthValue(builder, vOffset) {
            _LengthValue.startLengthValue(builder);
            _LengthValue.addV(builder, vOffset);
            return _LengthValue.endLengthValue(builder);
        }
        unpack() {
            return new LengthValueT(this.v() !== null ? this.v().unpack() : null);
        }
        unpackTo(_o) {
            _o.v = this.v() !== null ? this.v().unpack() : null;
        }
    };
    var LengthValueT = class {
        constructor(v = null){
            this.v = v;
        }
        v;
        pack(builder) {
            return LengthValue.createLengthValue(builder, this.v !== null ? this.v.pack(builder) : 0);
        }
    };
    var TextValue = class _TextValue {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsTextValue(bb, obj) {
            return (obj || new _TextValue).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsTextValue(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _TextValue).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        v(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startTextValue(builder) {
            builder.startObject(1);
        }
        static addV(builder, vOffset) {
            builder.addFieldOffset(0, vOffset, 0);
        }
        static endTextValue(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createTextValue(builder, vOffset) {
            _TextValue.startTextValue(builder);
            _TextValue.addV(builder, vOffset);
            return _TextValue.endTextValue(builder);
        }
        unpack() {
            return new TextValueT(this.v());
        }
        unpackTo(_o) {
            _o.v = this.v();
        }
    };
    var TextValueT = class {
        constructor(v = null){
            this.v = v;
        }
        v;
        pack(builder) {
            const v = this.v !== null ? builder.createString(this.v) : 0;
            return TextValue.createTextValue(builder, v);
        }
    };
    var PropValue = ((PropValue2)=>{
        PropValue2[PropValue2["NONE"] = 0] = "NONE";
        PropValue2[PropValue2["Length"] = 1] = "Length";
        PropValue2[PropValue2["ColorToken"] = 2] = "ColorToken";
        PropValue2[PropValue2["FontToken"] = 3] = "FontToken";
        PropValue2[PropValue2["Alignment"] = 4] = "Alignment";
        PropValue2[PropValue2["Axis"] = 5] = "Axis";
        PropValue2[PropValue2["Bool"] = 6] = "Bool";
        PropValue2[PropValue2["Text"] = 7] = "Text";
        return PropValue2;
    })(PropValue || {});
    function unionToPropValue(type, accessor) {
        switch(PropValue[type]){
            case "NONE":
                return null;
            case "Length":
                return accessor(new LengthValue);
            case "ColorToken":
                return accessor(new ColorToken);
            case "FontToken":
                return accessor(new FontToken);
            case "Alignment":
                return accessor(new AlignmentValue);
            case "Axis":
                return accessor(new AxisValue);
            case "Bool":
                return accessor(new BoolValue);
            case "Text":
                return accessor(new TextValue);
            default:
                return null;
        }
    }
    function unionListToPropValue(type, accessor, index) {
        switch(PropValue[type]){
            case "NONE":
                return null;
            case "Length":
                return accessor(index, new LengthValue);
            case "ColorToken":
                return accessor(index, new ColorToken);
            case "FontToken":
                return accessor(index, new FontToken);
            case "Alignment":
                return accessor(index, new AlignmentValue);
            case "Axis":
                return accessor(index, new AxisValue);
            case "Bool":
                return accessor(index, new BoolValue);
            case "Text":
                return accessor(index, new TextValue);
            default:
                return null;
        }
    }
    var BehaviourArg = class _BehaviourArg {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsBehaviourArg(bb, obj) {
            return (obj || new _BehaviourArg).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsBehaviourArg(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _BehaviourArg).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        name(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        valueType() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        value(obj) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__union(obj, this.bb_pos + offset) : null;
        }
        static startBehaviourArg(builder) {
            builder.startObject(3);
        }
        static addName(builder, nameOffset) {
            builder.addFieldOffset(0, nameOffset, 0);
        }
        static addValueType(builder, valueType) {
            builder.addFieldInt8(1, valueType, 0);
        }
        static addValue(builder, valueOffset) {
            builder.addFieldOffset(2, valueOffset, 0);
        }
        static endBehaviourArg(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createBehaviourArg(builder, nameOffset, valueType, valueOffset) {
            _BehaviourArg.startBehaviourArg(builder);
            _BehaviourArg.addName(builder, nameOffset);
            _BehaviourArg.addValueType(builder, valueType);
            _BehaviourArg.addValue(builder, valueOffset);
            return _BehaviourArg.endBehaviourArg(builder);
        }
        unpack() {
            return new BehaviourArgT(this.name(), this.valueType(), (()=>{
                const temp = unionToPropValue(this.valueType(), this.value.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })());
        }
        unpackTo(_o) {
            _o.name = this.name();
            _o.valueType = this.valueType();
            _o.value = (()=>{
                const temp = unionToPropValue(this.valueType(), this.value.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })();
        }
    };
    var BehaviourArgT = class {
        constructor(name = null, valueType = 0, value = null){
            this.name = name;
            this.valueType = valueType;
            this.value = value;
        }
        name;
        valueType;
        value;
        pack(builder) {
            const name = this.name !== null ? builder.createString(this.name) : 0;
            const value = builder.createObjectOffset(this.value);
            return BehaviourArg.createBehaviourArg(builder, name, this.valueType, value);
        }
    };
    var BehaviourId = ((BehaviourId2)=>{
        BehaviourId2[BehaviourId2["UNKNOWN"] = 0] = "UNKNOWN";
        BehaviourId2[BehaviourId2["DIALOG"] = 1] = "DIALOG";
        BehaviourId2[BehaviourId2["DISMISS"] = 2] = "DISMISS";
        BehaviourId2[BehaviourId2["SCROLL_LOCK"] = 3] = "SCROLL_LOCK";
        BehaviourId2[BehaviourId2["TRANSITION"] = 4] = "TRANSITION";
        return BehaviourId2;
    })(BehaviourId || {});
    var EventId = ((EventId2)=>{
        EventId2[EventId2["UNKNOWN"] = 0] = "UNKNOWN";
        EventId2[EventId2["TAP"] = 1] = "TAP";
        EventId2[EventId2["CHANGE"] = 2] = "CHANGE";
        EventId2[EventId2["SUBMIT"] = 3] = "SUBMIT";
        EventId2[EventId2["FOCUS"] = 4] = "FOCUS";
        EventId2[EventId2["BLUR"] = 5] = "BLUR";
        return EventId2;
    })(EventId || {});
    var BindAction = class _BindAction {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsBindAction(bb, obj) {
            return (obj || new _BindAction).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsBindAction(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _BindAction).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        event() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        action(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startBindAction(builder) {
            builder.startObject(3);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addEvent(builder, event) {
            builder.addFieldInt16(1, event, 0);
        }
        static addAction(builder, actionOffset) {
            builder.addFieldOffset(2, actionOffset, 0);
        }
        static endBindAction(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createBindAction(builder, nodeId, event, actionOffset) {
            _BindAction.startBindAction(builder);
            _BindAction.addNodeId(builder, nodeId);
            _BindAction.addEvent(builder, event);
            _BindAction.addAction(builder, actionOffset);
            return _BindAction.endBindAction(builder);
        }
        unpack() {
            return new BindActionT(this.nodeId(), this.event(), this.action());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.event = this.event();
            _o.action = this.action();
        }
    };
    var BindActionT = class {
        constructor(nodeId = 0, event = 0, action = null){
            this.nodeId = nodeId;
            this.event = event;
            this.action = action;
        }
        nodeId;
        event;
        action;
        pack(builder) {
            const action = this.action !== null ? builder.createString(this.action) : 0;
            return BindAction.createBindAction(builder, this.nodeId, this.event, action);
        }
    };
    var Bools = class _Bools {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsBools(bb, obj) {
            return (obj || new _Bools).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsBools(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _Bools).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        values(index) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? !!this.bb.readInt8(this.bb.__vector(this.bb_pos + offset) + index) : false;
        }
        valuesLength() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        valuesArray() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? new Int8Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        static startBools(builder) {
            builder.startObject(1);
        }
        static addValues(builder, valuesOffset) {
            builder.addFieldOffset(0, valuesOffset, 0);
        }
        static createValuesVector(builder, data) {
            builder.startVector(1, data.length, 1);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt8(+data[i]);
            }
            return builder.endVector();
        }
        static startValuesVector(builder, numElems) {
            builder.startVector(1, numElems, 1);
        }
        static endBools(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createBools(builder, valuesOffset) {
            _Bools.startBools(builder);
            _Bools.addValues(builder, valuesOffset);
            return _Bools.endBools(builder);
        }
        unpack() {
            return new BoolsT(this.bb.createScalarList(this.values.bind(this), this.valuesLength()));
        }
        unpackTo(_o) {
            _o.values = this.bb.createScalarList(this.values.bind(this), this.valuesLength());
        }
    };
    var BoolsT = class {
        constructor(values = []){
            this.values = values;
        }
        values;
        pack(builder) {
            const values = Bools.createValuesVector(builder, this.values);
            return Bools.createBools(builder, values);
        }
    };
    var CapsStatus = ((CapsStatus2)=>{
        CapsStatus2[CapsStatus2["Ok"] = 0] = "Ok";
        CapsStatus2[CapsStatus2["UpdateRequired"] = 1] = "UpdateRequired";
        return CapsStatus2;
    })(CapsStatus || {});
    var MountSurface = ((MountSurface2)=>{
        MountSurface2[MountSurface2["Native"] = 0] = "Native";
        MountSurface2[MountSurface2["Web"] = 1] = "Web";
        return MountSurface2;
    })(MountSurface || {});
    var Platform = ((Platform2)=>{
        Platform2[Platform2["Unknown"] = 0] = "Unknown";
        Platform2[Platform2["Web"] = 1] = "Web";
        Platform2[Platform2["Ios"] = 2] = "Ios";
        Platform2[Platform2["Android"] = 3] = "Android";
        Platform2[Platform2["Desktop"] = 4] = "Desktop";
        return Platform2;
    })(Platform || {});
    var ClientHello = class _ClientHello {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsClientHello(bb, obj) {
            return (obj || new _ClientHello).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsClientHello(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _ClientHello).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        vocabularyVersion() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        platform() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        mount() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        resumeFrom() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        subscriptionResume() {
            const offset = this.bb.__offset(this.bb_pos, 12);
            return offset ? !!this.bb.readInt8(this.bb_pos + offset) : false;
        }
        requestTag() {
            const offset = this.bb.__offset(this.bb_pos, 14);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        static startClientHello(builder) {
            builder.startObject(6);
        }
        static addVocabularyVersion(builder, vocabularyVersion) {
            builder.addFieldInt32(0, vocabularyVersion, 0);
        }
        static addPlatform(builder, platform) {
            builder.addFieldInt8(1, platform, 0);
        }
        static addMount(builder, mount) {
            builder.addFieldInt8(2, mount, 0);
        }
        static addResumeFrom(builder, resumeFrom) {
            builder.addFieldInt64(3, resumeFrom, BigInt("0"));
        }
        static addSubscriptionResume(builder, subscriptionResume) {
            builder.addFieldInt8(4, +subscriptionResume, 0);
        }
        static addRequestTag(builder, requestTag) {
            builder.addFieldInt64(5, requestTag, BigInt("0"));
        }
        static endClientHello(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createClientHello(builder, vocabularyVersion, platform, mount, resumeFrom, subscriptionResume, requestTag) {
            _ClientHello.startClientHello(builder);
            _ClientHello.addVocabularyVersion(builder, vocabularyVersion);
            _ClientHello.addPlatform(builder, platform);
            _ClientHello.addMount(builder, mount);
            _ClientHello.addResumeFrom(builder, resumeFrom);
            _ClientHello.addSubscriptionResume(builder, subscriptionResume);
            _ClientHello.addRequestTag(builder, requestTag);
            return _ClientHello.endClientHello(builder);
        }
        unpack() {
            return new ClientHelloT(this.vocabularyVersion(), this.platform(), this.mount(), this.resumeFrom(), this.subscriptionResume(), this.requestTag());
        }
        unpackTo(_o) {
            _o.vocabularyVersion = this.vocabularyVersion();
            _o.platform = this.platform();
            _o.mount = this.mount();
            _o.resumeFrom = this.resumeFrom();
            _o.subscriptionResume = this.subscriptionResume();
            _o.requestTag = this.requestTag();
        }
    };
    var ClientHelloT = class {
        constructor(vocabularyVersion = 0, platform = 0, mount = 0, resumeFrom = BigInt("0"), subscriptionResume = false, requestTag = BigInt("0")){
            this.vocabularyVersion = vocabularyVersion;
            this.platform = platform;
            this.mount = mount;
            this.resumeFrom = resumeFrom;
            this.subscriptionResume = subscriptionResume;
            this.requestTag = requestTag;
        }
        vocabularyVersion;
        platform;
        mount;
        resumeFrom;
        subscriptionResume;
        requestTag;
        pack(builder) {
            return ClientHello.createClientHello(builder, this.vocabularyVersion, this.platform, this.mount, this.resumeFrom, this.subscriptionResume, this.requestTag);
        }
    };
    var F64s = class _F64s {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsF64s(bb, obj) {
            return (obj || new _F64s).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsF64s(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _F64s).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        values(index) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readFloat64(this.bb.__vector(this.bb_pos + offset) + index * 8) : 0;
        }
        valuesLength() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        valuesArray() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? new Float64Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        static startF64s(builder) {
            builder.startObject(1);
        }
        static addValues(builder, valuesOffset) {
            builder.addFieldOffset(0, valuesOffset, 0);
        }
        static createValuesVector(builder, data) {
            builder.startVector(8, data.length, 8);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addFloat64(data[i]);
            }
            return builder.endVector();
        }
        static startValuesVector(builder, numElems) {
            builder.startVector(8, numElems, 8);
        }
        static endF64s(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createF64s(builder, valuesOffset) {
            _F64s.startF64s(builder);
            _F64s.addValues(builder, valuesOffset);
            return _F64s.endF64s(builder);
        }
        unpack() {
            return new F64sT(this.bb.createScalarList(this.values.bind(this), this.valuesLength()));
        }
        unpackTo(_o) {
            _o.values = this.bb.createScalarList(this.values.bind(this), this.valuesLength());
        }
    };
    var F64sT = class {
        constructor(values = []){
            this.values = values;
        }
        values;
        pack(builder) {
            const values = F64s.createValuesVector(builder, this.values);
            return F64s.createF64s(builder, values);
        }
    };
    var I32s = class _I32s {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsI32s(bb, obj) {
            return (obj || new _I32s).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsI32s(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _I32s).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        values(index) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readInt32(this.bb.__vector(this.bb_pos + offset) + index * 4) : 0;
        }
        valuesLength() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        valuesArray() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? new Int32Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        static startI32s(builder) {
            builder.startObject(1);
        }
        static addValues(builder, valuesOffset) {
            builder.addFieldOffset(0, valuesOffset, 0);
        }
        static createValuesVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt32(data[i]);
            }
            return builder.endVector();
        }
        static startValuesVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static endI32s(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createI32s(builder, valuesOffset) {
            _I32s.startI32s(builder);
            _I32s.addValues(builder, valuesOffset);
            return _I32s.endI32s(builder);
        }
        unpack() {
            return new I32sT(this.bb.createScalarList(this.values.bind(this), this.valuesLength()));
        }
        unpackTo(_o) {
            _o.values = this.bb.createScalarList(this.values.bind(this), this.valuesLength());
        }
    };
    var I32sT = class {
        constructor(values = []){
            this.values = values;
        }
        values;
        pack(builder) {
            const values = I32s.createValuesVector(builder, this.values);
            return I32s.createI32s(builder, values);
        }
    };
    var I64s = class _I64s {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsI64s(bb, obj) {
            return (obj || new _I64s).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsI64s(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _I64s).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        values(index) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readInt64(this.bb.__vector(this.bb_pos + offset) + index * 8) : BigInt(0);
        }
        valuesLength() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        static startI64s(builder) {
            builder.startObject(1);
        }
        static addValues(builder, valuesOffset) {
            builder.addFieldOffset(0, valuesOffset, 0);
        }
        static createValuesVector(builder, data) {
            builder.startVector(8, data.length, 8);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt64(data[i]);
            }
            return builder.endVector();
        }
        static startValuesVector(builder, numElems) {
            builder.startVector(8, numElems, 8);
        }
        static endI64s(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createI64s(builder, valuesOffset) {
            _I64s.startI64s(builder);
            _I64s.addValues(builder, valuesOffset);
            return _I64s.endI64s(builder);
        }
        unpack() {
            return new I64sT(this.bb.createScalarList(this.values.bind(this), this.valuesLength()));
        }
        unpackTo(_o) {
            _o.values = this.bb.createScalarList(this.values.bind(this), this.valuesLength());
        }
    };
    var I64sT = class {
        constructor(values = []){
            this.values = values;
        }
        values;
        pack(builder) {
            const values = I64s.createValuesVector(builder, this.values);
            return I64s.createI64s(builder, values);
        }
    };
    var Strings = class _Strings {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsStrings(bb, obj) {
            return (obj || new _Strings).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsStrings(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _Strings).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        values(index, optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.__string(this.bb.__vector(this.bb_pos + offset) + index * 4, optionalEncoding) : null;
        }
        valuesLength() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        static startStrings(builder) {
            builder.startObject(1);
        }
        static addValues(builder, valuesOffset) {
            builder.addFieldOffset(0, valuesOffset, 0);
        }
        static createValuesVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addOffset(data[i]);
            }
            return builder.endVector();
        }
        static startValuesVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static endStrings(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createStrings(builder, valuesOffset) {
            _Strings.startStrings(builder);
            _Strings.addValues(builder, valuesOffset);
            return _Strings.endStrings(builder);
        }
        unpack() {
            return new StringsT(this.bb.createScalarList(this.values.bind(this), this.valuesLength()));
        }
        unpackTo(_o) {
            _o.values = this.bb.createScalarList(this.values.bind(this), this.valuesLength());
        }
    };
    var StringsT = class {
        constructor(values = []){
            this.values = values;
        }
        values;
        pack(builder) {
            const values = Strings.createValuesVector(builder, builder.createObjectOffsetList(this.values));
            return Strings.createStrings(builder, values);
        }
    };
    var ColumnData = ((ColumnData2)=>{
        ColumnData2[ColumnData2["NONE"] = 0] = "NONE";
        ColumnData2[ColumnData2["I32"] = 1] = "I32";
        ColumnData2[ColumnData2["I64"] = 2] = "I64";
        ColumnData2[ColumnData2["F64"] = 3] = "F64";
        ColumnData2[ColumnData2["Bool"] = 4] = "Bool";
        ColumnData2[ColumnData2["Utf8"] = 5] = "Utf8";
        return ColumnData2;
    })(ColumnData || {});
    function unionToColumnData(type, accessor) {
        switch(ColumnData[type]){
            case "NONE":
                return null;
            case "I32":
                return accessor(new I32s);
            case "I64":
                return accessor(new I64s);
            case "F64":
                return accessor(new F64s);
            case "Bool":
                return accessor(new Bools);
            case "Utf8":
                return accessor(new Strings);
            default:
                return null;
        }
    }
    function unionListToColumnData(type, accessor, index) {
        switch(ColumnData[type]){
            case "NONE":
                return null;
            case "I32":
                return accessor(index, new I32s);
            case "I64":
                return accessor(index, new I64s);
            case "F64":
                return accessor(index, new F64s);
            case "Bool":
                return accessor(index, new Bools);
            case "Utf8":
                return accessor(index, new Strings);
            default:
                return null;
        }
    }
    var Column = class _Column {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsColumn(bb, obj) {
            return (obj || new _Column).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsColumn(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _Column).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        name(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        dataType() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        data(obj) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__union(obj, this.bb_pos + offset) : null;
        }
        validity(index) {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.readUint8(this.bb.__vector(this.bb_pos + offset) + index) : 0;
        }
        validityLength() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        validityArray() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? new Uint8Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        static startColumn(builder) {
            builder.startObject(4);
        }
        static addName(builder, nameOffset) {
            builder.addFieldOffset(0, nameOffset, 0);
        }
        static addDataType(builder, dataType) {
            builder.addFieldInt8(1, dataType, 0);
        }
        static addData(builder, dataOffset) {
            builder.addFieldOffset(2, dataOffset, 0);
        }
        static addValidity(builder, validityOffset) {
            builder.addFieldOffset(3, validityOffset, 0);
        }
        static createValidityVector(builder, data) {
            builder.startVector(1, data.length, 1);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt8(data[i]);
            }
            return builder.endVector();
        }
        static startValidityVector(builder, numElems) {
            builder.startVector(1, numElems, 1);
        }
        static endColumn(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createColumn(builder, nameOffset, dataType, dataOffset, validityOffset) {
            _Column.startColumn(builder);
            _Column.addName(builder, nameOffset);
            _Column.addDataType(builder, dataType);
            _Column.addData(builder, dataOffset);
            _Column.addValidity(builder, validityOffset);
            return _Column.endColumn(builder);
        }
        unpack() {
            return new ColumnT(this.name(), this.dataType(), (()=>{
                const temp = unionToColumnData(this.dataType(), this.data.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })(), this.bb.createScalarList(this.validity.bind(this), this.validityLength()));
        }
        unpackTo(_o) {
            _o.name = this.name();
            _o.dataType = this.dataType();
            _o.data = (()=>{
                const temp = unionToColumnData(this.dataType(), this.data.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })();
            _o.validity = this.bb.createScalarList(this.validity.bind(this), this.validityLength());
        }
    };
    var ColumnT = class {
        constructor(name = null, dataType = 0, data = null, validity = []){
            this.name = name;
            this.dataType = dataType;
            this.data = data;
            this.validity = validity;
        }
        name;
        dataType;
        data;
        validity;
        pack(builder) {
            const name = this.name !== null ? builder.createString(this.name) : 0;
            const data = builder.createObjectOffset(this.data);
            const validity = Column.createValidityVector(builder, this.validity);
            return Column.createColumn(builder, name, this.dataType, data, validity);
        }
    };
    var ColumnarBatch = class _ColumnarBatch {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsColumnarBatch(bb, obj) {
            return (obj || new _ColumnarBatch).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsColumnarBatch(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _ColumnarBatch).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        opCodes(index) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint8(this.bb.__vector(this.bb_pos + offset) + index) : 0;
        }
        opCodesLength() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        opCodesArray() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? new Uint8Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        nodeIds(index) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint32(this.bb.__vector(this.bb_pos + offset) + index * 4) : 0;
        }
        nodeIdsLength() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        nodeIdsArray() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? new Uint32Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        argA(index) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.readUint32(this.bb.__vector(this.bb_pos + offset) + index * 4) : 0;
        }
        argALength() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        argAArray() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? new Uint32Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        argB(index) {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.readUint32(this.bb.__vector(this.bb_pos + offset) + index * 4) : 0;
        }
        argBLength() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        argBArray() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? new Uint32Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        blobAOff(index) {
            const offset = this.bb.__offset(this.bb_pos, 12);
            return offset ? this.bb.readUint32(this.bb.__vector(this.bb_pos + offset) + index * 4) : 0;
        }
        blobAOffLength() {
            const offset = this.bb.__offset(this.bb_pos, 12);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        blobAOffArray() {
            const offset = this.bb.__offset(this.bb_pos, 12);
            return offset ? new Uint32Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        blobBOff(index) {
            const offset = this.bb.__offset(this.bb_pos, 14);
            return offset ? this.bb.readUint32(this.bb.__vector(this.bb_pos + offset) + index * 4) : 0;
        }
        blobBOffLength() {
            const offset = this.bb.__offset(this.bb_pos, 14);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        blobBOffArray() {
            const offset = this.bb.__offset(this.bb_pos, 14);
            return offset ? new Uint32Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        blobBytes(index) {
            const offset = this.bb.__offset(this.bb_pos, 16);
            return offset ? this.bb.readUint8(this.bb.__vector(this.bb_pos + offset) + index) : 0;
        }
        blobBytesLength() {
            const offset = this.bb.__offset(this.bb_pos, 16);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        blobBytesArray() {
            const offset = this.bb.__offset(this.bb_pos, 16);
            return offset ? new Uint8Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        static startColumnarBatch(builder) {
            builder.startObject(7);
        }
        static addOpCodes(builder, opCodesOffset) {
            builder.addFieldOffset(0, opCodesOffset, 0);
        }
        static createOpCodesVector(builder, data) {
            builder.startVector(1, data.length, 1);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt8(data[i]);
            }
            return builder.endVector();
        }
        static startOpCodesVector(builder, numElems) {
            builder.startVector(1, numElems, 1);
        }
        static addNodeIds(builder, nodeIdsOffset) {
            builder.addFieldOffset(1, nodeIdsOffset, 0);
        }
        static createNodeIdsVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt32(data[i]);
            }
            return builder.endVector();
        }
        static startNodeIdsVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static addArgA(builder, argAOffset) {
            builder.addFieldOffset(2, argAOffset, 0);
        }
        static createArgAVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt32(data[i]);
            }
            return builder.endVector();
        }
        static startArgAVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static addArgB(builder, argBOffset) {
            builder.addFieldOffset(3, argBOffset, 0);
        }
        static createArgBVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt32(data[i]);
            }
            return builder.endVector();
        }
        static startArgBVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static addBlobAOff(builder, blobAOffOffset) {
            builder.addFieldOffset(4, blobAOffOffset, 0);
        }
        static createBlobAOffVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt32(data[i]);
            }
            return builder.endVector();
        }
        static startBlobAOffVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static addBlobBOff(builder, blobBOffOffset) {
            builder.addFieldOffset(5, blobBOffOffset, 0);
        }
        static createBlobBOffVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt32(data[i]);
            }
            return builder.endVector();
        }
        static startBlobBOffVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static addBlobBytes(builder, blobBytesOffset) {
            builder.addFieldOffset(6, blobBytesOffset, 0);
        }
        static createBlobBytesVector(builder, data) {
            builder.startVector(1, data.length, 1);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt8(data[i]);
            }
            return builder.endVector();
        }
        static startBlobBytesVector(builder, numElems) {
            builder.startVector(1, numElems, 1);
        }
        static endColumnarBatch(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createColumnarBatch(builder, opCodesOffset, nodeIdsOffset, argAOffset, argBOffset, blobAOffOffset, blobBOffOffset, blobBytesOffset) {
            _ColumnarBatch.startColumnarBatch(builder);
            _ColumnarBatch.addOpCodes(builder, opCodesOffset);
            _ColumnarBatch.addNodeIds(builder, nodeIdsOffset);
            _ColumnarBatch.addArgA(builder, argAOffset);
            _ColumnarBatch.addArgB(builder, argBOffset);
            _ColumnarBatch.addBlobAOff(builder, blobAOffOffset);
            _ColumnarBatch.addBlobBOff(builder, blobBOffOffset);
            _ColumnarBatch.addBlobBytes(builder, blobBytesOffset);
            return _ColumnarBatch.endColumnarBatch(builder);
        }
        unpack() {
            return new ColumnarBatchT(this.bb.createScalarList(this.opCodes.bind(this), this.opCodesLength()), this.bb.createScalarList(this.nodeIds.bind(this), this.nodeIdsLength()), this.bb.createScalarList(this.argA.bind(this), this.argALength()), this.bb.createScalarList(this.argB.bind(this), this.argBLength()), this.bb.createScalarList(this.blobAOff.bind(this), this.blobAOffLength()), this.bb.createScalarList(this.blobBOff.bind(this), this.blobBOffLength()), this.bb.createScalarList(this.blobBytes.bind(this), this.blobBytesLength()));
        }
        unpackTo(_o) {
            _o.opCodes = this.bb.createScalarList(this.opCodes.bind(this), this.opCodesLength());
            _o.nodeIds = this.bb.createScalarList(this.nodeIds.bind(this), this.nodeIdsLength());
            _o.argA = this.bb.createScalarList(this.argA.bind(this), this.argALength());
            _o.argB = this.bb.createScalarList(this.argB.bind(this), this.argBLength());
            _o.blobAOff = this.bb.createScalarList(this.blobAOff.bind(this), this.blobAOffLength());
            _o.blobBOff = this.bb.createScalarList(this.blobBOff.bind(this), this.blobBOffLength());
            _o.blobBytes = this.bb.createScalarList(this.blobBytes.bind(this), this.blobBytesLength());
        }
    };
    var ColumnarBatchT = class {
        constructor(opCodes = [], nodeIds = [], argA = [], argB = [], blobAOff = [], blobBOff = [], blobBytes = []){
            this.opCodes = opCodes;
            this.nodeIds = nodeIds;
            this.argA = argA;
            this.argB = argB;
            this.blobAOff = blobAOff;
            this.blobBOff = blobBOff;
            this.blobBytes = blobBytes;
        }
        opCodes;
        nodeIds;
        argA;
        argB;
        blobAOff;
        blobBOff;
        blobBytes;
        pack(builder) {
            const opCodes = ColumnarBatch.createOpCodesVector(builder, this.opCodes);
            const nodeIds = ColumnarBatch.createNodeIdsVector(builder, this.nodeIds);
            const argA = ColumnarBatch.createArgAVector(builder, this.argA);
            const argB = ColumnarBatch.createArgBVector(builder, this.argB);
            const blobAOff = ColumnarBatch.createBlobAOffVector(builder, this.blobAOff);
            const blobBOff = ColumnarBatch.createBlobBOffVector(builder, this.blobBOff);
            const blobBytes = ColumnarBatch.createBlobBytesVector(builder, this.blobBytes);
            return ColumnarBatch.createColumnarBatch(builder, opCodes, nodeIds, argA, argB, blobAOff, blobBOff, blobBytes);
        }
    };
    var ConnectionSharing = ((ConnectionSharing2)=>{
        ConnectionSharing2[ConnectionSharing2["TransportDefault"] = 0] = "TransportDefault";
        ConnectionSharing2[ConnectionSharing2["Shared"] = 1] = "Shared";
        ConnectionSharing2[ConnectionSharing2["Dedicated"] = 2] = "Dedicated";
        return ConnectionSharing2;
    })(ConnectionSharing || {});
    var ReplayExpired = class _ReplayExpired {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsReplayExpired(bb, obj) {
            return (obj || new _ReplayExpired).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsReplayExpired(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _ReplayExpired).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        streamId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        after() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        static startReplayExpired(builder) {
            builder.startObject(2);
        }
        static addStreamId(builder, streamId) {
            builder.addFieldInt64(0, streamId, BigInt("0"));
        }
        static addAfter(builder, after) {
            builder.addFieldInt64(1, after, BigInt("0"));
        }
        static endReplayExpired(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createReplayExpired(builder, streamId, after) {
            _ReplayExpired.startReplayExpired(builder);
            _ReplayExpired.addStreamId(builder, streamId);
            _ReplayExpired.addAfter(builder, after);
            return _ReplayExpired.endReplayExpired(builder);
        }
        unpack() {
            return new ReplayExpiredT(this.streamId(), this.after());
        }
        unpackTo(_o) {
            _o.streamId = this.streamId();
            _o.after = this.after();
        }
    };
    var ReplayExpiredT = class {
        constructor(streamId = BigInt("0"), after = BigInt("0")){
            this.streamId = streamId;
            this.after = after;
        }
        streamId;
        after;
        pack(builder) {
            return ReplayExpired.createReplayExpired(builder, this.streamId, this.after);
        }
    };
    var ServerAction = class _ServerAction {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsServerAction(bb, obj) {
            return (obj || new _ServerAction).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsServerAction(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _ServerAction).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        hash() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        name(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startServerAction(builder) {
            builder.startObject(2);
        }
        static addHash(builder, hash) {
            builder.addFieldInt64(0, hash, BigInt("0"));
        }
        static addName(builder, nameOffset) {
            builder.addFieldOffset(1, nameOffset, 0);
        }
        static endServerAction(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createServerAction(builder, hash, nameOffset) {
            _ServerAction.startServerAction(builder);
            _ServerAction.addHash(builder, hash);
            _ServerAction.addName(builder, nameOffset);
            return _ServerAction.endServerAction(builder);
        }
        unpack() {
            return new ServerActionT(this.hash(), this.name());
        }
        unpackTo(_o) {
            _o.hash = this.hash();
            _o.name = this.name();
        }
    };
    var ServerActionT = class {
        constructor(hash = BigInt("0"), name = null){
            this.hash = hash;
            this.name = name;
        }
        hash;
        name;
        pack(builder) {
            const name = this.name !== null ? builder.createString(this.name) : 0;
            return ServerAction.createServerAction(builder, this.hash, name);
        }
    };
    var ServerCaps = class _ServerCaps {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsServerCaps(bb, obj) {
            return (obj || new _ServerCaps).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsServerCaps(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _ServerCaps).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        status() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        schemaVersion() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        vocabularyVersion() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        streamId() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        resumeFrom() {
            const offset = this.bb.__offset(this.bb_pos, 12);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        actions(index, obj) {
            const offset = this.bb.__offset(this.bb_pos, 14);
            return offset ? (obj || new ServerAction).__init(this.bb.__indirect(this.bb.__vector(this.bb_pos + offset) + index * 4), this.bb) : null;
        }
        actionsLength() {
            const offset = this.bb.__offset(this.bb_pos, 14);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        subscriptionResume() {
            const offset = this.bb.__offset(this.bb_pos, 16);
            return offset ? !!this.bb.readInt8(this.bb_pos + offset) : false;
        }
        requestTag() {
            const offset = this.bb.__offset(this.bb_pos, 18);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        static startServerCaps(builder) {
            builder.startObject(8);
        }
        static addStatus(builder, status) {
            builder.addFieldInt8(0, status, 0);
        }
        static addSchemaVersion(builder, schemaVersion) {
            builder.addFieldInt16(1, schemaVersion, 0);
        }
        static addVocabularyVersion(builder, vocabularyVersion) {
            builder.addFieldInt32(2, vocabularyVersion, 0);
        }
        static addStreamId(builder, streamId) {
            builder.addFieldInt64(3, streamId, BigInt("0"));
        }
        static addResumeFrom(builder, resumeFrom) {
            builder.addFieldInt64(4, resumeFrom, BigInt("0"));
        }
        static addActions(builder, actionsOffset) {
            builder.addFieldOffset(5, actionsOffset, 0);
        }
        static createActionsVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addOffset(data[i]);
            }
            return builder.endVector();
        }
        static startActionsVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static addSubscriptionResume(builder, subscriptionResume) {
            builder.addFieldInt8(6, +subscriptionResume, 0);
        }
        static addRequestTag(builder, requestTag) {
            builder.addFieldInt64(7, requestTag, BigInt("0"));
        }
        static endServerCaps(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createServerCaps(builder, status, schemaVersion, vocabularyVersion, streamId, resumeFrom, actionsOffset, subscriptionResume, requestTag) {
            _ServerCaps.startServerCaps(builder);
            _ServerCaps.addStatus(builder, status);
            _ServerCaps.addSchemaVersion(builder, schemaVersion);
            _ServerCaps.addVocabularyVersion(builder, vocabularyVersion);
            _ServerCaps.addStreamId(builder, streamId);
            _ServerCaps.addResumeFrom(builder, resumeFrom);
            _ServerCaps.addActions(builder, actionsOffset);
            _ServerCaps.addSubscriptionResume(builder, subscriptionResume);
            _ServerCaps.addRequestTag(builder, requestTag);
            return _ServerCaps.endServerCaps(builder);
        }
        unpack() {
            return new ServerCapsT(this.status(), this.schemaVersion(), this.vocabularyVersion(), this.streamId(), this.resumeFrom(), this.bb.createObjList(this.actions.bind(this), this.actionsLength()), this.subscriptionResume(), this.requestTag());
        }
        unpackTo(_o) {
            _o.status = this.status();
            _o.schemaVersion = this.schemaVersion();
            _o.vocabularyVersion = this.vocabularyVersion();
            _o.streamId = this.streamId();
            _o.resumeFrom = this.resumeFrom();
            _o.actions = this.bb.createObjList(this.actions.bind(this), this.actionsLength());
            _o.subscriptionResume = this.subscriptionResume();
            _o.requestTag = this.requestTag();
        }
    };
    var ServerCapsT = class {
        constructor(status = 0, schemaVersion = 0, vocabularyVersion = 0, streamId = BigInt("0"), resumeFrom = BigInt("0"), actions = [], subscriptionResume = false, requestTag = BigInt("0")){
            this.status = status;
            this.schemaVersion = schemaVersion;
            this.vocabularyVersion = vocabularyVersion;
            this.streamId = streamId;
            this.resumeFrom = resumeFrom;
            this.actions = actions;
            this.subscriptionResume = subscriptionResume;
            this.requestTag = requestTag;
        }
        status;
        schemaVersion;
        vocabularyVersion;
        streamId;
        resumeFrom;
        actions;
        subscriptionResume;
        requestTag;
        pack(builder) {
            const actions = ServerCaps.createActionsVector(builder, builder.createObjectOffsetList(this.actions));
            return ServerCaps.createServerCaps(builder, this.status, this.schemaVersion, this.vocabularyVersion, this.streamId, this.resumeFrom, actions, this.subscriptionResume, this.requestTag);
        }
    };
    var StateExpired = class _StateExpired {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsStateExpired(bb, obj) {
            return (obj || new _StateExpired).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsStateExpired(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _StateExpired).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        tokenIssuedAt() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        static startStateExpired(builder) {
            builder.startObject(1);
        }
        static addTokenIssuedAt(builder, tokenIssuedAt) {
            builder.addFieldInt64(0, tokenIssuedAt, BigInt("0"));
        }
        static endStateExpired(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createStateExpired(builder, tokenIssuedAt) {
            _StateExpired.startStateExpired(builder);
            _StateExpired.addTokenIssuedAt(builder, tokenIssuedAt);
            return _StateExpired.endStateExpired(builder);
        }
        unpack() {
            return new StateExpiredT(this.tokenIssuedAt());
        }
        unpackTo(_o) {
            _o.tokenIssuedAt = this.tokenIssuedAt();
        }
    };
    var StateExpiredT = class {
        constructor(tokenIssuedAt = BigInt("0")){
            this.tokenIssuedAt = tokenIssuedAt;
        }
        tokenIssuedAt;
        pack(builder) {
            return StateExpired.createStateExpired(builder, this.tokenIssuedAt);
        }
    };
    var UpdateRequired = class _UpdateRequired {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsUpdateRequired(bb, obj) {
            return (obj || new _UpdateRequired).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsUpdateRequired(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _UpdateRequired).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        clientSchemaVersion() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        serverSchemaVersion() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        static startUpdateRequired(builder) {
            builder.startObject(2);
        }
        static addClientSchemaVersion(builder, clientSchemaVersion) {
            builder.addFieldInt16(0, clientSchemaVersion, 0);
        }
        static addServerSchemaVersion(builder, serverSchemaVersion) {
            builder.addFieldInt16(1, serverSchemaVersion, 0);
        }
        static endUpdateRequired(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createUpdateRequired(builder, clientSchemaVersion, serverSchemaVersion) {
            _UpdateRequired.startUpdateRequired(builder);
            _UpdateRequired.addClientSchemaVersion(builder, clientSchemaVersion);
            _UpdateRequired.addServerSchemaVersion(builder, serverSchemaVersion);
            return _UpdateRequired.endUpdateRequired(builder);
        }
        unpack() {
            return new UpdateRequiredT(this.clientSchemaVersion(), this.serverSchemaVersion());
        }
        unpackTo(_o) {
            _o.clientSchemaVersion = this.clientSchemaVersion();
            _o.serverSchemaVersion = this.serverSchemaVersion();
        }
    };
    var UpdateRequiredT = class {
        constructor(clientSchemaVersion = 0, serverSchemaVersion = 0){
            this.clientSchemaVersion = clientSchemaVersion;
            this.serverSchemaVersion = serverSchemaVersion;
        }
        clientSchemaVersion;
        serverSchemaVersion;
        pack(builder) {
            return UpdateRequired.createUpdateRequired(builder, this.clientSchemaVersion, this.serverSchemaVersion);
        }
    };
    var Control = class _Control {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsControl(bb, obj) {
            return (obj || new _Control).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsControl(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _Control).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        caps(obj) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? (obj || new ServerCaps).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        stateExpired(obj) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? (obj || new StateExpired).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        updateRequired(obj) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? (obj || new UpdateRequired).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        error(obj) {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? (obj || new ActionError).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        replayExpired(obj) {
            const offset = this.bb.__offset(this.bb_pos, 12);
            return offset ? (obj || new ReplayExpired).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        static startControl(builder) {
            builder.startObject(5);
        }
        static addCaps(builder, capsOffset) {
            builder.addFieldOffset(0, capsOffset, 0);
        }
        static addStateExpired(builder, stateExpiredOffset) {
            builder.addFieldOffset(1, stateExpiredOffset, 0);
        }
        static addUpdateRequired(builder, updateRequiredOffset) {
            builder.addFieldOffset(2, updateRequiredOffset, 0);
        }
        static addError(builder, errorOffset) {
            builder.addFieldOffset(3, errorOffset, 0);
        }
        static addReplayExpired(builder, replayExpiredOffset) {
            builder.addFieldOffset(4, replayExpiredOffset, 0);
        }
        static endControl(builder) {
            const offset = builder.endObject();
            return offset;
        }
        unpack() {
            return new ControlT(this.caps() !== null ? this.caps().unpack() : null, this.stateExpired() !== null ? this.stateExpired().unpack() : null, this.updateRequired() !== null ? this.updateRequired().unpack() : null, this.error() !== null ? this.error().unpack() : null, this.replayExpired() !== null ? this.replayExpired().unpack() : null);
        }
        unpackTo(_o) {
            _o.caps = this.caps() !== null ? this.caps().unpack() : null;
            _o.stateExpired = this.stateExpired() !== null ? this.stateExpired().unpack() : null;
            _o.updateRequired = this.updateRequired() !== null ? this.updateRequired().unpack() : null;
            _o.error = this.error() !== null ? this.error().unpack() : null;
            _o.replayExpired = this.replayExpired() !== null ? this.replayExpired().unpack() : null;
        }
    };
    var ControlT = class {
        constructor(caps = null, stateExpired = null, updateRequired = null, error = null, replayExpired = null){
            this.caps = caps;
            this.stateExpired = stateExpired;
            this.updateRequired = updateRequired;
            this.error = error;
            this.replayExpired = replayExpired;
        }
        caps;
        stateExpired;
        updateRequired;
        error;
        replayExpired;
        pack(builder) {
            const caps = this.caps !== null ? this.caps.pack(builder) : 0;
            const stateExpired = this.stateExpired !== null ? this.stateExpired.pack(builder) : 0;
            const updateRequired = this.updateRequired !== null ? this.updateRequired.pack(builder) : 0;
            const error = this.error !== null ? this.error.pack(builder) : 0;
            const replayExpired = this.replayExpired !== null ? this.replayExpired.pack(builder) : 0;
            Control.startControl(builder);
            Control.addCaps(builder, caps);
            Control.addStateExpired(builder, stateExpired);
            Control.addUpdateRequired(builder, updateRequired);
            Control.addError(builder, error);
            Control.addReplayExpired(builder, replayExpired);
            return Control.endControl(builder);
        }
    };
    var NodeKind = ((NodeKind2)=>{
        NodeKind2[NodeKind2["UNKNOWN"] = 0] = "UNKNOWN";
        NodeKind2[NodeKind2["STACK"] = 1] = "STACK";
        NodeKind2[NodeKind2["TEXT"] = 2] = "TEXT";
        NodeKind2[NodeKind2["IMAGE"] = 3] = "IMAGE";
        NodeKind2[NodeKind2["BUTTON"] = 4] = "BUTTON";
        NodeKind2[NodeKind2["TEXT_INPUT"] = 5] = "TEXT_INPUT";
        NodeKind2[NodeKind2["TOGGLE"] = 6] = "TOGGLE";
        NodeKind2[NodeKind2["LIST"] = 7] = "LIST";
        NodeKind2[NodeKind2["SCROLL"] = 8] = "SCROLL";
        NodeKind2[NodeKind2["SPACER"] = 9] = "SPACER";
        NodeKind2[NodeKind2["OVERLAY"] = 10] = "OVERLAY";
        NodeKind2[NodeKind2["MODAL"] = 11] = "MODAL";
        NodeKind2[NodeKind2["BOTTOM_SHEET"] = 12] = "BOTTOM_SHEET";
        NodeKind2[NodeKind2["NAVIGATION_TITLE"] = 13] = "NAVIGATION_TITLE";
        NodeKind2[NodeKind2["TOOLBAR_BUTTON"] = 14] = "TOOLBAR_BUTTON";
        NodeKind2[NodeKind2["SEGMENTED_CONTROL"] = 15] = "SEGMENTED_CONTROL";
        NodeKind2[NodeKind2["INSET_LIST"] = 16] = "INSET_LIST";
        NodeKind2[NodeKind2["CHECKLIST_ROW"] = 17] = "CHECKLIST_ROW";
        NodeKind2[NodeKind2["CHECKMARK_BUTTON"] = 18] = "CHECKMARK_BUTTON";
        NodeKind2[NodeKind2["DESTRUCTIVE_BUTTON"] = 19] = "DESTRUCTIVE_BUTTON";
        NodeKind2[NodeKind2["LIST_FOOTER"] = 20] = "LIST_FOOTER";
        NodeKind2[NodeKind2["CONFIRMATION_DIALOG"] = 21] = "CONFIRMATION_DIALOG";
        NodeKind2[NodeKind2["SHEET_TITLE"] = 22] = "SHEET_TITLE";
        NodeKind2[NodeKind2["SUBMIT_BUTTON"] = 23] = "SUBMIT_BUTTON";
        NodeKind2[NodeKind2["CANCEL_BUTTON"] = 24] = "CANCEL_BUTTON";
        NodeKind2[NodeKind2["DRAWING_SURFACE"] = 25] = "DRAWING_SURFACE";
        return NodeKind2;
    })(NodeKind || {});
    var CreateNode = class _CreateNode {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsCreateNode(bb, obj) {
            return (obj || new _CreateNode).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsCreateNode(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _CreateNode).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        kind() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        static startCreateNode(builder) {
            builder.startObject(2);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addKind(builder, kind) {
            builder.addFieldInt16(1, kind, 0);
        }
        static endCreateNode(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createCreateNode(builder, nodeId, kind) {
            _CreateNode.startCreateNode(builder);
            _CreateNode.addNodeId(builder, nodeId);
            _CreateNode.addKind(builder, kind);
            return _CreateNode.endCreateNode(builder);
        }
        unpack() {
            return new CreateNodeT(this.nodeId(), this.kind());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.kind = this.kind();
        }
    };
    var CreateNodeT = class {
        constructor(nodeId = 0, kind = 0){
            this.nodeId = nodeId;
            this.kind = kind;
        }
        nodeId;
        kind;
        pack(builder) {
            return CreateNode.createCreateNode(builder, this.nodeId, this.kind);
        }
    };
    var DataTable = class _DataTable {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsDataTable(bb, obj) {
            return (obj || new _DataTable).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsDataTable(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _DataTable).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        targetNode() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        rows() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        columns(index, obj) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? (obj || new Column).__init(this.bb.__indirect(this.bb.__vector(this.bb_pos + offset) + index * 4), this.bb) : null;
        }
        columnsLength() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        static startDataTable(builder) {
            builder.startObject(3);
        }
        static addTargetNode(builder, targetNode) {
            builder.addFieldInt32(0, targetNode, 0);
        }
        static addRows(builder, rows) {
            builder.addFieldInt64(1, rows, BigInt("0"));
        }
        static addColumns(builder, columnsOffset) {
            builder.addFieldOffset(2, columnsOffset, 0);
        }
        static createColumnsVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addOffset(data[i]);
            }
            return builder.endVector();
        }
        static startColumnsVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static endDataTable(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createDataTable(builder, targetNode, rows, columnsOffset) {
            _DataTable.startDataTable(builder);
            _DataTable.addTargetNode(builder, targetNode);
            _DataTable.addRows(builder, rows);
            _DataTable.addColumns(builder, columnsOffset);
            return _DataTable.endDataTable(builder);
        }
        unpack() {
            return new DataTableT(this.targetNode(), this.rows(), this.bb.createObjList(this.columns.bind(this), this.columnsLength()));
        }
        unpackTo(_o) {
            _o.targetNode = this.targetNode();
            _o.rows = this.rows();
            _o.columns = this.bb.createObjList(this.columns.bind(this), this.columnsLength());
        }
    };
    var DataTableT = class {
        constructor(targetNode = 0, rows = BigInt("0"), columns = []){
            this.targetNode = targetNode;
            this.rows = rows;
            this.columns = columns;
        }
        targetNode;
        rows;
        columns;
        pack(builder) {
            const columns = DataTable.createColumnsVector(builder, builder.createObjectOffsetList(this.columns));
            return DataTable.createDataTable(builder, this.targetNode, this.rows, columns);
        }
    };
    var MessagePackBlob = class _MessagePackBlob {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsMessagePackBlob(bb, obj) {
            return (obj || new _MessagePackBlob).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsMessagePackBlob(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _MessagePackBlob).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        targetNode() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        bytes(index) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint8(this.bb.__vector(this.bb_pos + offset) + index) : 0;
        }
        bytesLength() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        bytesArray() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? new Uint8Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        static startMessagePackBlob(builder) {
            builder.startObject(2);
        }
        static addTargetNode(builder, targetNode) {
            builder.addFieldInt32(0, targetNode, 0);
        }
        static addBytes(builder, bytesOffset) {
            builder.addFieldOffset(1, bytesOffset, 0);
        }
        static createBytesVector(builder, data) {
            builder.startVector(1, data.length, 1);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt8(data[i]);
            }
            return builder.endVector();
        }
        static startBytesVector(builder, numElems) {
            builder.startVector(1, numElems, 1);
        }
        static endMessagePackBlob(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createMessagePackBlob(builder, targetNode, bytesOffset) {
            _MessagePackBlob.startMessagePackBlob(builder);
            _MessagePackBlob.addTargetNode(builder, targetNode);
            _MessagePackBlob.addBytes(builder, bytesOffset);
            return _MessagePackBlob.endMessagePackBlob(builder);
        }
        unpack() {
            return new MessagePackBlobT(this.targetNode(), this.bb.createScalarList(this.bytes.bind(this), this.bytesLength()));
        }
        unpackTo(_o) {
            _o.targetNode = this.targetNode();
            _o.bytes = this.bb.createScalarList(this.bytes.bind(this), this.bytesLength());
        }
    };
    var MessagePackBlobT = class {
        constructor(targetNode = 0, bytes = []){
            this.targetNode = targetNode;
            this.bytes = bytes;
        }
        targetNode;
        bytes;
        pack(builder) {
            const bytes = MessagePackBlob.createBytesVector(builder, this.bytes);
            return MessagePackBlob.createMessagePackBlob(builder, this.targetNode, bytes);
        }
    };
    var Data = class _Data {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsData(bb, obj) {
            return (obj || new _Data).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsData(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _Data).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        targetNode() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        arrow(obj) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? (obj || new ArrowIpcBlob).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        table(obj) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? (obj || new DataTable).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        msgpack(obj) {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? (obj || new MessagePackBlob).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        static startData(builder) {
            builder.startObject(4);
        }
        static addTargetNode(builder, targetNode) {
            builder.addFieldInt32(0, targetNode, 0);
        }
        static addArrow(builder, arrowOffset) {
            builder.addFieldOffset(1, arrowOffset, 0);
        }
        static addTable(builder, tableOffset) {
            builder.addFieldOffset(2, tableOffset, 0);
        }
        static addMsgpack(builder, msgpackOffset) {
            builder.addFieldOffset(3, msgpackOffset, 0);
        }
        static endData(builder) {
            const offset = builder.endObject();
            return offset;
        }
        unpack() {
            return new DataT(this.targetNode(), this.arrow() !== null ? this.arrow().unpack() : null, this.table() !== null ? this.table().unpack() : null, this.msgpack() !== null ? this.msgpack().unpack() : null);
        }
        unpackTo(_o) {
            _o.targetNode = this.targetNode();
            _o.arrow = this.arrow() !== null ? this.arrow().unpack() : null;
            _o.table = this.table() !== null ? this.table().unpack() : null;
            _o.msgpack = this.msgpack() !== null ? this.msgpack().unpack() : null;
        }
    };
    var DataT = class {
        constructor(targetNode = 0, arrow = null, table = null, msgpack = null){
            this.targetNode = targetNode;
            this.arrow = arrow;
            this.table = table;
            this.msgpack = msgpack;
        }
        targetNode;
        arrow;
        table;
        msgpack;
        pack(builder) {
            const arrow = this.arrow !== null ? this.arrow.pack(builder) : 0;
            const table = this.table !== null ? this.table.pack(builder) : 0;
            const msgpack = this.msgpack !== null ? this.msgpack.pack(builder) : 0;
            Data.startData(builder);
            Data.addTargetNode(builder, this.targetNode);
            Data.addArrow(builder, arrow);
            Data.addTable(builder, table);
            Data.addMsgpack(builder, msgpack);
            return Data.endData(builder);
        }
    };
    var MobileBehaviourId = ((MobileBehaviourId2)=>{
        MobileBehaviourId2[MobileBehaviourId2["UNKNOWN"] = 0] = "UNKNOWN";
        return MobileBehaviourId2;
    })(MobileBehaviourId || {});
    var MobileBehaviour = class _MobileBehaviour {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsMobileBehaviour(bb, obj) {
            return (obj || new _MobileBehaviour).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsMobileBehaviour(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _MobileBehaviour).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        behaviour() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        custom(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        args(index, obj) {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? (obj || new BehaviourArg).__init(this.bb.__indirect(this.bb.__vector(this.bb_pos + offset) + index * 4), this.bb) : null;
        }
        argsLength() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        static startMobileBehaviour(builder) {
            builder.startObject(4);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addBehaviour(builder, behaviour) {
            builder.addFieldInt16(1, behaviour, 0);
        }
        static addCustom(builder, customOffset) {
            builder.addFieldOffset(2, customOffset, 0);
        }
        static addArgs(builder, argsOffset) {
            builder.addFieldOffset(3, argsOffset, 0);
        }
        static createArgsVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addOffset(data[i]);
            }
            return builder.endVector();
        }
        static startArgsVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static endMobileBehaviour(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createMobileBehaviour(builder, nodeId, behaviour, customOffset, argsOffset) {
            _MobileBehaviour.startMobileBehaviour(builder);
            _MobileBehaviour.addNodeId(builder, nodeId);
            _MobileBehaviour.addBehaviour(builder, behaviour);
            _MobileBehaviour.addCustom(builder, customOffset);
            _MobileBehaviour.addArgs(builder, argsOffset);
            return _MobileBehaviour.endMobileBehaviour(builder);
        }
        unpack() {
            return new MobileBehaviourT(this.nodeId(), this.behaviour(), this.custom(), this.bb.createObjList(this.args.bind(this), this.argsLength()));
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.behaviour = this.behaviour();
            _o.custom = this.custom();
            _o.args = this.bb.createObjList(this.args.bind(this), this.argsLength());
        }
    };
    var MobileBehaviourT = class {
        constructor(nodeId = 0, behaviour = 0, custom = null, args = []){
            this.nodeId = nodeId;
            this.behaviour = behaviour;
            this.custom = custom;
            this.args = args;
        }
        nodeId;
        behaviour;
        custom;
        args;
        pack(builder) {
            const custom = this.custom !== null ? builder.createString(this.custom) : 0;
            const args = MobileBehaviour.createArgsVector(builder, builder.createObjectOffsetList(this.args));
            return MobileBehaviour.createMobileBehaviour(builder, this.nodeId, this.behaviour, custom, args);
        }
    };
    var MountWebView = class _MountWebView {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsMountWebView(bb, obj) {
            return (obj || new _MountWebView).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsMountWebView(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _MountWebView).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        parentId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        url(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startMountWebView(builder) {
            builder.startObject(3);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addParentId(builder, parentId) {
            builder.addFieldInt32(1, parentId, 0);
        }
        static addUrl(builder, urlOffset) {
            builder.addFieldOffset(2, urlOffset, 0);
        }
        static endMountWebView(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createMountWebView(builder, nodeId, parentId, urlOffset) {
            _MountWebView.startMountWebView(builder);
            _MountWebView.addNodeId(builder, nodeId);
            _MountWebView.addParentId(builder, parentId);
            _MountWebView.addUrl(builder, urlOffset);
            return _MountWebView.endMountWebView(builder);
        }
        unpack() {
            return new MountWebViewT(this.nodeId(), this.parentId(), this.url());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.parentId = this.parentId();
            _o.url = this.url();
        }
    };
    var MountWebViewT = class {
        constructor(nodeId = 0, parentId = 0, url = null){
            this.nodeId = nodeId;
            this.parentId = parentId;
            this.url = url;
        }
        nodeId;
        parentId;
        url;
        pack(builder) {
            const url = this.url !== null ? builder.createString(this.url) : 0;
            return MountWebView.createMountWebView(builder, this.nodeId, this.parentId, url);
        }
    };
    var NativeArg = class _NativeArg {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsNativeArg(bb, obj) {
            return (obj || new _NativeArg).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsNativeArg(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _NativeArg).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        name(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        valueType() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        value(obj) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__union(obj, this.bb_pos + offset) : null;
        }
        static startNativeArg(builder) {
            builder.startObject(3);
        }
        static addName(builder, nameOffset) {
            builder.addFieldOffset(0, nameOffset, 0);
        }
        static addValueType(builder, valueType) {
            builder.addFieldInt8(1, valueType, 0);
        }
        static addValue(builder, valueOffset) {
            builder.addFieldOffset(2, valueOffset, 0);
        }
        static endNativeArg(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createNativeArg(builder, nameOffset, valueType, valueOffset) {
            _NativeArg.startNativeArg(builder);
            _NativeArg.addName(builder, nameOffset);
            _NativeArg.addValueType(builder, valueType);
            _NativeArg.addValue(builder, valueOffset);
            return _NativeArg.endNativeArg(builder);
        }
        unpack() {
            return new NativeArgT(this.name(), this.valueType(), (()=>{
                const temp = unionToPropValue(this.valueType(), this.value.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })());
        }
        unpackTo(_o) {
            _o.name = this.name();
            _o.valueType = this.valueType();
            _o.value = (()=>{
                const temp = unionToPropValue(this.valueType(), this.value.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })();
        }
    };
    var NativeArgT = class {
        constructor(name = null, valueType = 0, value = null){
            this.name = name;
            this.valueType = valueType;
            this.value = value;
        }
        name;
        valueType;
        value;
        pack(builder) {
            const name = this.name !== null ? builder.createString(this.name) : 0;
            const value = builder.createObjectOffset(this.value);
            return NativeArg.createNativeArg(builder, name, this.valueType, value);
        }
    };
    var NativeCommandId = ((NativeCommandId2)=>{
        NativeCommandId2[NativeCommandId2["UNKNOWN"] = 0] = "UNKNOWN";
        NativeCommandId2[NativeCommandId2["HAPTIC"] = 1] = "HAPTIC";
        NativeCommandId2[NativeCommandId2["OPEN_URL"] = 2] = "OPEN_URL";
        NativeCommandId2[NativeCommandId2["SHARE"] = 3] = "SHARE";
        return NativeCommandId2;
    })(NativeCommandId || {});
    var NativeCommand = class _NativeCommand {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsNativeCommand(bb, obj) {
            return (obj || new _NativeCommand).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsNativeCommand(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _NativeCommand).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        command() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        args(index, obj) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? (obj || new NativeArg).__init(this.bb.__indirect(this.bb.__vector(this.bb_pos + offset) + index * 4), this.bb) : null;
        }
        argsLength() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        static startNativeCommand(builder) {
            builder.startObject(2);
        }
        static addCommand(builder, command) {
            builder.addFieldInt16(0, command, 0);
        }
        static addArgs(builder, argsOffset) {
            builder.addFieldOffset(1, argsOffset, 0);
        }
        static createArgsVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addOffset(data[i]);
            }
            return builder.endVector();
        }
        static startArgsVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static endNativeCommand(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createNativeCommand(builder, command, argsOffset) {
            _NativeCommand.startNativeCommand(builder);
            _NativeCommand.addCommand(builder, command);
            _NativeCommand.addArgs(builder, argsOffset);
            return _NativeCommand.endNativeCommand(builder);
        }
        unpack() {
            return new NativeCommandT(this.command(), this.bb.createObjList(this.args.bind(this), this.argsLength()));
        }
        unpackTo(_o) {
            _o.command = this.command();
            _o.args = this.bb.createObjList(this.args.bind(this), this.argsLength());
        }
    };
    var NativeCommandT = class {
        constructor(command = 0, args = []){
            this.command = command;
            this.args = args;
        }
        command;
        args;
        pack(builder) {
            const args = NativeCommand.createArgsVector(builder, builder.createObjectOffsetList(this.args));
            return NativeCommand.createNativeCommand(builder, this.command, args);
        }
    };
    var MobileOperation = ((MobileOperation2)=>{
        MobileOperation2[MobileOperation2["NONE"] = 0] = "NONE";
        MobileOperation2[MobileOperation2["MountWebView"] = 1] = "MountWebView";
        MobileOperation2[MobileOperation2["NativeCommand"] = 2] = "NativeCommand";
        MobileOperation2[MobileOperation2["MobileBehaviour"] = 3] = "MobileBehaviour";
        return MobileOperation2;
    })(MobileOperation || {});
    function unionToMobileOperation(type, accessor) {
        switch(MobileOperation[type]){
            case "NONE":
                return null;
            case "MountWebView":
                return accessor(new MountWebView);
            case "NativeCommand":
                return accessor(new NativeCommand);
            case "MobileBehaviour":
                return accessor(new MobileBehaviour);
            default:
                return null;
        }
    }
    function unionListToMobileOperation(type, accessor, index) {
        switch(MobileOperation[type]){
            case "NONE":
                return null;
            case "MountWebView":
                return accessor(index, new MountWebView);
            case "NativeCommand":
                return accessor(index, new NativeCommand);
            case "MobileBehaviour":
                return accessor(index, new MobileBehaviour);
            default:
                return null;
        }
    }
    var InsertBefore = class _InsertBefore {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsInsertBefore(bb, obj) {
            return (obj || new _InsertBefore).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsInsertBefore(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _InsertBefore).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        parentId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        childId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        beforeId() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        static startInsertBefore(builder) {
            builder.startObject(3);
        }
        static addParentId(builder, parentId) {
            builder.addFieldInt32(0, parentId, 0);
        }
        static addChildId(builder, childId) {
            builder.addFieldInt32(1, childId, 0);
        }
        static addBeforeId(builder, beforeId) {
            builder.addFieldInt32(2, beforeId, 0);
        }
        static endInsertBefore(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createInsertBefore(builder, parentId, childId, beforeId) {
            _InsertBefore.startInsertBefore(builder);
            _InsertBefore.addParentId(builder, parentId);
            _InsertBefore.addChildId(builder, childId);
            _InsertBefore.addBeforeId(builder, beforeId);
            return _InsertBefore.endInsertBefore(builder);
        }
        unpack() {
            return new InsertBeforeT(this.parentId(), this.childId(), this.beforeId());
        }
        unpackTo(_o) {
            _o.parentId = this.parentId();
            _o.childId = this.childId();
            _o.beforeId = this.beforeId();
        }
    };
    var InsertBeforeT = class {
        constructor(parentId = 0, childId = 0, beforeId = 0){
            this.parentId = parentId;
            this.childId = childId;
            this.beforeId = beforeId;
        }
        parentId;
        childId;
        beforeId;
        pack(builder) {
            return InsertBefore.createInsertBefore(builder, this.parentId, this.childId, this.beforeId);
        }
    };
    var LiveKind = ((LiveKind2)=>{
        LiveKind2[LiveKind2["Stream"] = 0] = "Stream";
        LiveKind2[LiveKind2["Data"] = 1] = "Data";
        return LiveKind2;
    })(LiveKind || {});
    var LiveTransport = ((LiveTransport2)=>{
        LiveTransport2[LiveTransport2["Local"] = 0] = "Local";
        LiveTransport2[LiveTransport2["Sse"] = 1] = "Sse";
        LiveTransport2[LiveTransport2["WebSocket"] = 2] = "WebSocket";
        LiveTransport2[LiveTransport2["Http"] = 3] = "Http";
        return LiveTransport2;
    })(LiveTransport || {});
    var LiveMount = class _LiveMount {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsLiveMount(bb, obj) {
            return (obj || new _LiveMount).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsLiveMount(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _LiveMount).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        kind() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        channelId() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        transport() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        endpoint(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 12);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        resumeFrom() {
            const offset = this.bb.__offset(this.bb_pos, 14);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        sharing() {
            const offset = this.bb.__offset(this.bb_pos, 16);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        keepAlive() {
            const offset = this.bb.__offset(this.bb_pos, 18);
            return offset ? !!this.bb.readInt8(this.bb_pos + offset) : false;
        }
        static startLiveMount(builder) {
            builder.startObject(8);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addKind(builder, kind) {
            builder.addFieldInt8(1, kind, 0);
        }
        static addChannelId(builder, channelId) {
            builder.addFieldInt64(2, channelId, BigInt("0"));
        }
        static addTransport(builder, transport) {
            builder.addFieldInt8(3, transport, 0);
        }
        static addEndpoint(builder, endpointOffset) {
            builder.addFieldOffset(4, endpointOffset, 0);
        }
        static addResumeFrom(builder, resumeFrom) {
            builder.addFieldInt64(5, resumeFrom, BigInt("0"));
        }
        static addSharing(builder, sharing) {
            builder.addFieldInt8(6, sharing, 0);
        }
        static addKeepAlive(builder, keepAlive) {
            builder.addFieldInt8(7, +keepAlive, 0);
        }
        static endLiveMount(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createLiveMount(builder, nodeId, kind, channelId, transport, endpointOffset, resumeFrom, sharing, keepAlive) {
            _LiveMount.startLiveMount(builder);
            _LiveMount.addNodeId(builder, nodeId);
            _LiveMount.addKind(builder, kind);
            _LiveMount.addChannelId(builder, channelId);
            _LiveMount.addTransport(builder, transport);
            _LiveMount.addEndpoint(builder, endpointOffset);
            _LiveMount.addResumeFrom(builder, resumeFrom);
            _LiveMount.addSharing(builder, sharing);
            _LiveMount.addKeepAlive(builder, keepAlive);
            return _LiveMount.endLiveMount(builder);
        }
        unpack() {
            return new LiveMountT(this.nodeId(), this.kind(), this.channelId(), this.transport(), this.endpoint(), this.resumeFrom(), this.sharing(), this.keepAlive());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.kind = this.kind();
            _o.channelId = this.channelId();
            _o.transport = this.transport();
            _o.endpoint = this.endpoint();
            _o.resumeFrom = this.resumeFrom();
            _o.sharing = this.sharing();
            _o.keepAlive = this.keepAlive();
        }
    };
    var LiveMountT = class {
        constructor(nodeId = 0, kind = 0, channelId = BigInt("0"), transport = 0, endpoint = null, resumeFrom = BigInt("0"), sharing = 0, keepAlive = false){
            this.nodeId = nodeId;
            this.kind = kind;
            this.channelId = channelId;
            this.transport = transport;
            this.endpoint = endpoint;
            this.resumeFrom = resumeFrom;
            this.sharing = sharing;
            this.keepAlive = keepAlive;
        }
        nodeId;
        kind;
        channelId;
        transport;
        endpoint;
        resumeFrom;
        sharing;
        keepAlive;
        pack(builder) {
            const endpoint = this.endpoint !== null ? builder.createString(this.endpoint) : 0;
            return LiveMount.createLiveMount(builder, this.nodeId, this.kind, this.channelId, this.transport, endpoint, this.resumeFrom, this.sharing, this.keepAlive);
        }
    };
    var PlacementKind = ((PlacementKind2)=>{
        PlacementKind2[PlacementKind2["Append"] = 0] = "Append";
        PlacementKind2[PlacementKind2["Before"] = 1] = "Before";
        PlacementKind2[PlacementKind2["After"] = 2] = "After";
        PlacementKind2[PlacementKind2["Swap"] = 3] = "Swap";
        PlacementKind2[PlacementKind2["Remove"] = 4] = "Remove";
        PlacementKind2[PlacementKind2["FullView"] = 5] = "FullView";
        return PlacementKind2;
    })(PlacementKind || {});
    var SelectorKind = ((SelectorKind2)=>{
        SelectorKind2[SelectorKind2["NodeId"] = 0] = "NodeId";
        SelectorKind2[SelectorKind2["Id"] = 1] = "Id";
        SelectorKind2[SelectorKind2["Class"] = 2] = "Class";
        SelectorKind2[SelectorKind2["Query"] = 3] = "Query";
        SelectorKind2[SelectorKind2["TypedId"] = 4] = "TypedId";
        SelectorKind2[SelectorKind2["View"] = 5] = "View";
        SelectorKind2[SelectorKind2["Receiving"] = 6] = "Receiving";
        return SelectorKind2;
    })(SelectorKind || {});
    var TargetSelector = class _TargetSelector {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsTargetSelector(bb, obj) {
            return (obj || new _TargetSelector).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsTargetSelector(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _TargetSelector).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        kind() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        text(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startTargetSelector(builder) {
            builder.startObject(3);
        }
        static addKind(builder, kind) {
            builder.addFieldInt8(0, kind, 0);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(1, nodeId, 0);
        }
        static addText(builder, textOffset) {
            builder.addFieldOffset(2, textOffset, 0);
        }
        static endTargetSelector(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createTargetSelector(builder, kind, nodeId, textOffset) {
            _TargetSelector.startTargetSelector(builder);
            _TargetSelector.addKind(builder, kind);
            _TargetSelector.addNodeId(builder, nodeId);
            _TargetSelector.addText(builder, textOffset);
            return _TargetSelector.endTargetSelector(builder);
        }
        unpack() {
            return new TargetSelectorT(this.kind(), this.nodeId(), this.text());
        }
        unpackTo(_o) {
            _o.kind = this.kind();
            _o.nodeId = this.nodeId();
            _o.text = this.text();
        }
    };
    var TargetSelectorT = class {
        constructor(kind = 0, nodeId = 0, text = null){
            this.kind = kind;
            this.nodeId = nodeId;
            this.text = text;
        }
        kind;
        nodeId;
        text;
        pack(builder) {
            const text = this.text !== null ? builder.createString(this.text) : 0;
            return TargetSelector.createTargetSelector(builder, this.kind, this.nodeId, text);
        }
    };
    var Place = class _Place {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsPlace(bb, obj) {
            return (obj || new _Place).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsPlace(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _Place).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        kind() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        target(obj) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? (obj || new TargetSelector).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        child(obj) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? (obj || new TargetSelector).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        rootNode() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        operations(index, obj) {
            const offset = this.bb.__offset(this.bb_pos, 12);
            return offset ? (obj || new Operation).__init(this.bb.__indirect(this.bb.__vector(this.bb_pos + offset) + index * 4), this.bb) : null;
        }
        operationsLength() {
            const offset = this.bb.__offset(this.bb_pos, 12);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        static startPlace(builder) {
            builder.startObject(5);
        }
        static addKind(builder, kind) {
            builder.addFieldInt8(0, kind, 0);
        }
        static addTarget(builder, targetOffset) {
            builder.addFieldOffset(1, targetOffset, 0);
        }
        static addChild(builder, childOffset) {
            builder.addFieldOffset(2, childOffset, 0);
        }
        static addRootNode(builder, rootNode) {
            builder.addFieldInt32(3, rootNode, 0);
        }
        static addOperations(builder, operationsOffset) {
            builder.addFieldOffset(4, operationsOffset, 0);
        }
        static createOperationsVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addOffset(data[i]);
            }
            return builder.endVector();
        }
        static startOperationsVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static endPlace(builder) {
            const offset = builder.endObject();
            return offset;
        }
        unpack() {
            return new PlaceT(this.kind(), this.target() !== null ? this.target().unpack() : null, this.child() !== null ? this.child().unpack() : null, this.rootNode(), this.bb.createObjList(this.operations.bind(this), this.operationsLength()));
        }
        unpackTo(_o) {
            _o.kind = this.kind();
            _o.target = this.target() !== null ? this.target().unpack() : null;
            _o.child = this.child() !== null ? this.child().unpack() : null;
            _o.rootNode = this.rootNode();
            _o.operations = this.bb.createObjList(this.operations.bind(this), this.operationsLength());
        }
    };
    var PlaceT = class {
        constructor(kind = 0, target = null, child = null, rootNode = 0, operations = []){
            this.kind = kind;
            this.target = target;
            this.child = child;
            this.rootNode = rootNode;
            this.operations = operations;
        }
        kind;
        target;
        child;
        rootNode;
        operations;
        pack(builder) {
            const target = this.target !== null ? this.target.pack(builder) : 0;
            const child = this.child !== null ? this.child.pack(builder) : 0;
            const operations = Place.createOperationsVector(builder, builder.createObjectOffsetList(this.operations));
            Place.startPlace(builder);
            Place.addKind(builder, this.kind);
            Place.addTarget(builder, target);
            Place.addChild(builder, child);
            Place.addRootNode(builder, this.rootNode);
            Place.addOperations(builder, operations);
            return Place.endPlace(builder);
        }
    };
    var Remove = class _Remove {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsRemove(bb, obj) {
            return (obj || new _Remove).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsRemove(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _Remove).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        static startRemove(builder) {
            builder.startObject(1);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static endRemove(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createRemove(builder, nodeId) {
            _Remove.startRemove(builder);
            _Remove.addNodeId(builder, nodeId);
            return _Remove.endRemove(builder);
        }
        unpack() {
            return new RemoveT(this.nodeId());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
        }
    };
    var RemoveT = class {
        constructor(nodeId = 0){
            this.nodeId = nodeId;
        }
        nodeId;
        pack(builder) {
            return Remove.createRemove(builder, this.nodeId);
        }
    };
    var ReplaceSubtree = class _ReplaceSubtree {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsReplaceSubtree(bb, obj) {
            return (obj || new _ReplaceSubtree).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsReplaceSubtree(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _ReplaceSubtree).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        operations(index, obj) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? (obj || new Operation).__init(this.bb.__indirect(this.bb.__vector(this.bb_pos + offset) + index * 4), this.bb) : null;
        }
        operationsLength() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        static startReplaceSubtree(builder) {
            builder.startObject(2);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addOperations(builder, operationsOffset) {
            builder.addFieldOffset(1, operationsOffset, 0);
        }
        static createOperationsVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addOffset(data[i]);
            }
            return builder.endVector();
        }
        static startOperationsVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static endReplaceSubtree(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createReplaceSubtree(builder, nodeId, operationsOffset) {
            _ReplaceSubtree.startReplaceSubtree(builder);
            _ReplaceSubtree.addNodeId(builder, nodeId);
            _ReplaceSubtree.addOperations(builder, operationsOffset);
            return _ReplaceSubtree.endReplaceSubtree(builder);
        }
        unpack() {
            return new ReplaceSubtreeT(this.nodeId(), this.bb.createObjList(this.operations.bind(this), this.operationsLength()));
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.operations = this.bb.createObjList(this.operations.bind(this), this.operationsLength());
        }
    };
    var ReplaceSubtreeT = class {
        constructor(nodeId = 0, operations = []){
            this.nodeId = nodeId;
            this.operations = operations;
        }
        nodeId;
        operations;
        pack(builder) {
            const operations = ReplaceSubtree.createOperationsVector(builder, builder.createObjectOffsetList(this.operations));
            return ReplaceSubtree.createReplaceSubtree(builder, this.nodeId, operations);
        }
    };
    var PropId = ((PropId2)=>{
        PropId2[PropId2["UNKNOWN"] = 0] = "UNKNOWN";
        PropId2[PropId2["PADDING"] = 1] = "PADDING";
        PropId2[PropId2["SPACING"] = 2] = "SPACING";
        PropId2[PropId2["ALIGN"] = 3] = "ALIGN";
        PropId2[PropId2["COLOR"] = 4] = "COLOR";
        PropId2[PropId2["FONT"] = 5] = "FONT";
        PropId2[PropId2["ENABLED"] = 6] = "ENABLED";
        PropId2[PropId2["A11Y_LABEL"] = 7] = "A11Y_LABEL";
        PropId2[PropId2["AXIS"] = 8] = "AXIS";
        PropId2[PropId2["VALUE"] = 9] = "VALUE";
        PropId2[PropId2["CHECKED"] = 10] = "CHECKED";
        PropId2[PropId2["SOURCE"] = 11] = "SOURCE";
        PropId2[PropId2["PLACEHOLDER"] = 12] = "PLACEHOLDER";
        PropId2[PropId2["BACKGROUND"] = 13] = "BACKGROUND";
        PropId2[PropId2["WIDTH"] = 14] = "WIDTH";
        PropId2[PropId2["HEIGHT"] = 15] = "HEIGHT";
        PropId2[PropId2["HIDDEN"] = 16] = "HIDDEN";
        PropId2[PropId2["PRESENTATION"] = 17] = "PRESENTATION";
        PropId2[PropId2["UTILITY_CLASSES"] = 18] = "UTILITY_CLASSES";
        PropId2[PropId2["ID"] = 19] = "ID";
        return PropId2;
    })(PropId || {});
    var SetProp = class _SetProp {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsSetProp(bb, obj) {
            return (obj || new _SetProp).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsSetProp(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _SetProp).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        prop() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        valueType() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        value(obj) {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.__union(obj, this.bb_pos + offset) : null;
        }
        static startSetProp(builder) {
            builder.startObject(4);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addProp(builder, prop) {
            builder.addFieldInt16(1, prop, 0);
        }
        static addValueType(builder, valueType) {
            builder.addFieldInt8(2, valueType, 0);
        }
        static addValue(builder, valueOffset) {
            builder.addFieldOffset(3, valueOffset, 0);
        }
        static endSetProp(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createSetProp(builder, nodeId, prop, valueType, valueOffset) {
            _SetProp.startSetProp(builder);
            _SetProp.addNodeId(builder, nodeId);
            _SetProp.addProp(builder, prop);
            _SetProp.addValueType(builder, valueType);
            _SetProp.addValue(builder, valueOffset);
            return _SetProp.endSetProp(builder);
        }
        unpack() {
            return new SetPropT(this.nodeId(), this.prop(), this.valueType(), (()=>{
                const temp = unionToPropValue(this.valueType(), this.value.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.prop = this.prop();
            _o.valueType = this.valueType();
            _o.value = (()=>{
                const temp = unionToPropValue(this.valueType(), this.value.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })();
        }
    };
    var SetPropT = class {
        constructor(nodeId = 0, prop = 0, valueType = 0, value = null){
            this.nodeId = nodeId;
            this.prop = prop;
            this.valueType = valueType;
            this.value = value;
        }
        nodeId;
        prop;
        valueType;
        value;
        pack(builder) {
            const value = builder.createObjectOffset(this.value);
            return SetProp.createSetProp(builder, this.nodeId, this.prop, this.valueType, value);
        }
    };
    var SetText = class _SetText {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsSetText(bb, obj) {
            return (obj || new _SetText).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsSetText(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _SetText).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        text(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startSetText(builder) {
            builder.startObject(2);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addText(builder, textOffset) {
            builder.addFieldOffset(1, textOffset, 0);
        }
        static endSetText(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createSetText(builder, nodeId, textOffset) {
            _SetText.startSetText(builder);
            _SetText.addNodeId(builder, nodeId);
            _SetText.addText(builder, textOffset);
            return _SetText.endSetText(builder);
        }
        unpack() {
            return new SetTextT(this.nodeId(), this.text());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.text = this.text();
        }
    };
    var SetTextT = class {
        constructor(nodeId = 0, text = null){
            this.nodeId = nodeId;
            this.text = text;
        }
        nodeId;
        text;
        pack(builder) {
            const text = this.text !== null ? builder.createString(this.text) : 0;
            return SetText.createSetText(builder, this.nodeId, text);
        }
    };
    var UiBehaviour = class _UiBehaviour {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsUiBehaviour(bb, obj) {
            return (obj || new _UiBehaviour).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsUiBehaviour(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _UiBehaviour).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        behaviour() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        args(index, obj) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? (obj || new BehaviourArg).__init(this.bb.__indirect(this.bb.__vector(this.bb_pos + offset) + index * 4), this.bb) : null;
        }
        argsLength() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        static startUiBehaviour(builder) {
            builder.startObject(3);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addBehaviour(builder, behaviour) {
            builder.addFieldInt16(1, behaviour, 0);
        }
        static addArgs(builder, argsOffset) {
            builder.addFieldOffset(2, argsOffset, 0);
        }
        static createArgsVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addOffset(data[i]);
            }
            return builder.endVector();
        }
        static startArgsVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static endUiBehaviour(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createUiBehaviour(builder, nodeId, behaviour, argsOffset) {
            _UiBehaviour.startUiBehaviour(builder);
            _UiBehaviour.addNodeId(builder, nodeId);
            _UiBehaviour.addBehaviour(builder, behaviour);
            _UiBehaviour.addArgs(builder, argsOffset);
            return _UiBehaviour.endUiBehaviour(builder);
        }
        unpack() {
            return new UiBehaviourT(this.nodeId(), this.behaviour(), this.bb.createObjList(this.args.bind(this), this.argsLength()));
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.behaviour = this.behaviour();
            _o.args = this.bb.createObjList(this.args.bind(this), this.argsLength());
        }
    };
    var UiBehaviourT = class {
        constructor(nodeId = 0, behaviour = 0, args = []){
            this.nodeId = nodeId;
            this.behaviour = behaviour;
            this.args = args;
        }
        nodeId;
        behaviour;
        args;
        pack(builder) {
            const args = UiBehaviour.createArgsVector(builder, builder.createObjectOffsetList(this.args));
            return UiBehaviour.createUiBehaviour(builder, this.nodeId, this.behaviour, args);
        }
    };
    var UnbindAction = class _UnbindAction {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsUnbindAction(bb, obj) {
            return (obj || new _UnbindAction).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsUnbindAction(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _UnbindAction).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        event() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        static startUnbindAction(builder) {
            builder.startObject(2);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addEvent(builder, event) {
            builder.addFieldInt16(1, event, 0);
        }
        static endUnbindAction(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createUnbindAction(builder, nodeId, event) {
            _UnbindAction.startUnbindAction(builder);
            _UnbindAction.addNodeId(builder, nodeId);
            _UnbindAction.addEvent(builder, event);
            return _UnbindAction.endUnbindAction(builder);
        }
        unpack() {
            return new UnbindActionT(this.nodeId(), this.event());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.event = this.event();
        }
    };
    var UnbindActionT = class {
        constructor(nodeId = 0, event = 0){
            this.nodeId = nodeId;
            this.event = event;
        }
        nodeId;
        event;
        pack(builder) {
            return UnbindAction.createUnbindAction(builder, this.nodeId, this.event);
        }
    };
    var UiOperation = ((UiOperation2)=>{
        UiOperation2[UiOperation2["NONE"] = 0] = "NONE";
        UiOperation2[UiOperation2["CreateNode"] = 1] = "CreateNode";
        UiOperation2[UiOperation2["SetText"] = 2] = "SetText";
        UiOperation2[UiOperation2["SetProp"] = 3] = "SetProp";
        UiOperation2[UiOperation2["AppendChild"] = 4] = "AppendChild";
        UiOperation2[UiOperation2["InsertBefore"] = 5] = "InsertBefore";
        UiOperation2[UiOperation2["Remove"] = 6] = "Remove";
        UiOperation2[UiOperation2["ReplaceSubtree"] = 7] = "ReplaceSubtree";
        UiOperation2[UiOperation2["BindAction"] = 8] = "BindAction";
        UiOperation2[UiOperation2["UnbindAction"] = 9] = "UnbindAction";
        UiOperation2[UiOperation2["AttachState"] = 10] = "AttachState";
        UiOperation2[UiOperation2["LiveMount"] = 11] = "LiveMount";
        UiOperation2[UiOperation2["UiBehaviour"] = 12] = "UiBehaviour";
        UiOperation2[UiOperation2["Place"] = 13] = "Place";
        return UiOperation2;
    })(UiOperation || {});
    function unionToUiOperation(type, accessor) {
        switch(UiOperation[type]){
            case "NONE":
                return null;
            case "CreateNode":
                return accessor(new CreateNode);
            case "SetText":
                return accessor(new SetText);
            case "SetProp":
                return accessor(new SetProp);
            case "AppendChild":
                return accessor(new AppendChild);
            case "InsertBefore":
                return accessor(new InsertBefore);
            case "Remove":
                return accessor(new Remove);
            case "ReplaceSubtree":
                return accessor(new ReplaceSubtree);
            case "BindAction":
                return accessor(new BindAction);
            case "UnbindAction":
                return accessor(new UnbindAction);
            case "AttachState":
                return accessor(new AttachState);
            case "LiveMount":
                return accessor(new LiveMount);
            case "UiBehaviour":
                return accessor(new UiBehaviour);
            case "Place":
                return accessor(new Place);
            default:
                return null;
        }
    }
    function unionListToUiOperation(type, accessor, index) {
        switch(UiOperation[type]){
            case "NONE":
                return null;
            case "CreateNode":
                return accessor(index, new CreateNode);
            case "SetText":
                return accessor(index, new SetText);
            case "SetProp":
                return accessor(index, new SetProp);
            case "AppendChild":
                return accessor(index, new AppendChild);
            case "InsertBefore":
                return accessor(index, new InsertBefore);
            case "Remove":
                return accessor(index, new Remove);
            case "ReplaceSubtree":
                return accessor(index, new ReplaceSubtree);
            case "BindAction":
                return accessor(index, new BindAction);
            case "UnbindAction":
                return accessor(index, new UnbindAction);
            case "AttachState":
                return accessor(index, new AttachState);
            case "LiveMount":
                return accessor(index, new LiveMount);
            case "UiBehaviour":
                return accessor(index, new UiBehaviour);
            case "Place":
                return accessor(index, new Place);
            default:
                return null;
        }
    }
    var MorphAction = ((MorphAction2)=>{
        MorphAction2[MorphAction2["ReplaceChildren"] = 0] = "ReplaceChildren";
        MorphAction2[MorphAction2["ReplaceElement"] = 1] = "ReplaceElement";
        MorphAction2[MorphAction2["InsertBefore"] = 2] = "InsertBefore";
        MorphAction2[MorphAction2["InsertAfter"] = 3] = "InsertAfter";
        MorphAction2[MorphAction2["AppendSibling"] = 4] = "AppendSibling";
        return MorphAction2;
    })(MorphAction || {});
    var HtmlMorph = class _HtmlMorph {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsHtmlMorph(bb, obj) {
            return (obj || new _HtmlMorph).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsHtmlMorph(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _HtmlMorph).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        target(obj) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? (obj || new TargetSelector).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        action() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        content(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startHtmlMorph(builder) {
            builder.startObject(3);
        }
        static addTarget(builder, targetOffset) {
            builder.addFieldOffset(0, targetOffset, 0);
        }
        static addAction(builder, action) {
            builder.addFieldInt8(1, action, 0);
        }
        static addContent(builder, contentOffset) {
            builder.addFieldOffset(2, contentOffset, 0);
        }
        static endHtmlMorph(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createHtmlMorph(builder, targetOffset, action, contentOffset) {
            _HtmlMorph.startHtmlMorph(builder);
            _HtmlMorph.addTarget(builder, targetOffset);
            _HtmlMorph.addAction(builder, action);
            _HtmlMorph.addContent(builder, contentOffset);
            return _HtmlMorph.endHtmlMorph(builder);
        }
        unpack() {
            return new HtmlMorphT(this.target() !== null ? this.target().unpack() : null, this.action(), this.content());
        }
        unpackTo(_o) {
            _o.target = this.target() !== null ? this.target().unpack() : null;
            _o.action = this.action();
            _o.content = this.content();
        }
    };
    var HtmlMorphT = class {
        constructor(target = null, action = 0, content = null){
            this.target = target;
            this.action = action;
            this.content = content;
        }
        target;
        action;
        content;
        pack(builder) {
            const target = this.target !== null ? this.target.pack(builder) : 0;
            const content = this.content !== null ? builder.createString(this.content) : 0;
            return HtmlMorph.createHtmlMorph(builder, target, this.action, content);
        }
    };
    var ScriptNode = class _ScriptNode {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsScriptNode(bb, obj) {
            return (obj || new _ScriptNode).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsScriptNode(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _ScriptNode).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        parentId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        body(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startScriptNode(builder) {
            builder.startObject(3);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addParentId(builder, parentId) {
            builder.addFieldInt32(1, parentId, 0);
        }
        static addBody(builder, bodyOffset) {
            builder.addFieldOffset(2, bodyOffset, 0);
        }
        static endScriptNode(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createScriptNode(builder, nodeId, parentId, bodyOffset) {
            _ScriptNode.startScriptNode(builder);
            _ScriptNode.addNodeId(builder, nodeId);
            _ScriptNode.addParentId(builder, parentId);
            _ScriptNode.addBody(builder, bodyOffset);
            return _ScriptNode.endScriptNode(builder);
        }
        unpack() {
            return new ScriptNodeT(this.nodeId(), this.parentId(), this.body());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.parentId = this.parentId();
            _o.body = this.body();
        }
    };
    var ScriptNodeT = class {
        constructor(nodeId = 0, parentId = 0, body = null){
            this.nodeId = nodeId;
            this.parentId = parentId;
            this.body = body;
        }
        nodeId;
        parentId;
        body;
        pack(builder) {
            const body = this.body !== null ? builder.createString(this.body) : 0;
            return ScriptNode.createScriptNode(builder, this.nodeId, this.parentId, body);
        }
    };
    var WebAddClass = class _WebAddClass {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebAddClass(bb, obj) {
            return (obj || new _WebAddClass).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebAddClass(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebAddClass).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        className(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startWebAddClass(builder) {
            builder.startObject(2);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addClassName(builder, classNameOffset) {
            builder.addFieldOffset(1, classNameOffset, 0);
        }
        static endWebAddClass(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebAddClass(builder, nodeId, classNameOffset) {
            _WebAddClass.startWebAddClass(builder);
            _WebAddClass.addNodeId(builder, nodeId);
            _WebAddClass.addClassName(builder, classNameOffset);
            return _WebAddClass.endWebAddClass(builder);
        }
        unpack() {
            return new WebAddClassT(this.nodeId(), this.className());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.className = this.className();
        }
    };
    var WebAddClassT = class {
        constructor(nodeId = 0, className = null){
            this.nodeId = nodeId;
            this.className = className;
        }
        nodeId;
        className;
        pack(builder) {
            const className = this.className !== null ? builder.createString(this.className) : 0;
            return WebAddClass.createWebAddClass(builder, this.nodeId, className);
        }
    };
    var WebAddListener = class _WebAddListener {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebAddListener(bb, obj) {
            return (obj || new _WebAddListener).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebAddListener(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebAddListener).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        nameId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        customName(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startWebAddListener(builder) {
            builder.startObject(3);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addNameId(builder, nameId) {
            builder.addFieldInt16(1, nameId, 0);
        }
        static addCustomName(builder, customNameOffset) {
            builder.addFieldOffset(2, customNameOffset, 0);
        }
        static endWebAddListener(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebAddListener(builder, nodeId, nameId, customNameOffset) {
            _WebAddListener.startWebAddListener(builder);
            _WebAddListener.addNodeId(builder, nodeId);
            _WebAddListener.addNameId(builder, nameId);
            _WebAddListener.addCustomName(builder, customNameOffset);
            return _WebAddListener.endWebAddListener(builder);
        }
        unpack() {
            return new WebAddListenerT(this.nodeId(), this.nameId(), this.customName());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.nameId = this.nameId();
            _o.customName = this.customName();
        }
    };
    var WebAddListenerT = class {
        constructor(nodeId = 0, nameId = 0, customName = null){
            this.nodeId = nodeId;
            this.nameId = nameId;
            this.customName = customName;
        }
        nodeId;
        nameId;
        customName;
        pack(builder) {
            const customName = this.customName !== null ? builder.createString(this.customName) : 0;
            return WebAddListener.createWebAddListener(builder, this.nodeId, this.nameId, customName);
        }
    };
    var WebBehaviourId = ((WebBehaviourId2)=>{
        WebBehaviourId2[WebBehaviourId2["UNKNOWN"] = 0] = "UNKNOWN";
        return WebBehaviourId2;
    })(WebBehaviourId || {});
    var WebBehaviour = class _WebBehaviour {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebBehaviour(bb, obj) {
            return (obj || new _WebBehaviour).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebBehaviour(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebBehaviour).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        behaviour() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        custom(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        args(index, obj) {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? (obj || new BehaviourArg).__init(this.bb.__indirect(this.bb.__vector(this.bb_pos + offset) + index * 4), this.bb) : null;
        }
        argsLength() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        static startWebBehaviour(builder) {
            builder.startObject(4);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addBehaviour(builder, behaviour) {
            builder.addFieldInt16(1, behaviour, 0);
        }
        static addCustom(builder, customOffset) {
            builder.addFieldOffset(2, customOffset, 0);
        }
        static addArgs(builder, argsOffset) {
            builder.addFieldOffset(3, argsOffset, 0);
        }
        static createArgsVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addOffset(data[i]);
            }
            return builder.endVector();
        }
        static startArgsVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static endWebBehaviour(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebBehaviour(builder, nodeId, behaviour, customOffset, argsOffset) {
            _WebBehaviour.startWebBehaviour(builder);
            _WebBehaviour.addNodeId(builder, nodeId);
            _WebBehaviour.addBehaviour(builder, behaviour);
            _WebBehaviour.addCustom(builder, customOffset);
            _WebBehaviour.addArgs(builder, argsOffset);
            return _WebBehaviour.endWebBehaviour(builder);
        }
        unpack() {
            return new WebBehaviourT(this.nodeId(), this.behaviour(), this.custom(), this.bb.createObjList(this.args.bind(this), this.argsLength()));
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.behaviour = this.behaviour();
            _o.custom = this.custom();
            _o.args = this.bb.createObjList(this.args.bind(this), this.argsLength());
        }
    };
    var WebBehaviourT = class {
        constructor(nodeId = 0, behaviour = 0, custom = null, args = []){
            this.nodeId = nodeId;
            this.behaviour = behaviour;
            this.custom = custom;
            this.args = args;
        }
        nodeId;
        behaviour;
        custom;
        args;
        pack(builder) {
            const custom = this.custom !== null ? builder.createString(this.custom) : 0;
            const args = WebBehaviour.createArgsVector(builder, builder.createObjectOffsetList(this.args));
            return WebBehaviour.createWebBehaviour(builder, this.nodeId, this.behaviour, custom, args);
        }
    };
    var WebDetachChild = class _WebDetachChild {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebDetachChild(bb, obj) {
            return (obj || new _WebDetachChild).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebDetachChild(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebDetachChild).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        parentId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        childId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        static startWebDetachChild(builder) {
            builder.startObject(2);
        }
        static addParentId(builder, parentId) {
            builder.addFieldInt32(0, parentId, 0);
        }
        static addChildId(builder, childId) {
            builder.addFieldInt32(1, childId, 0);
        }
        static endWebDetachChild(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebDetachChild(builder, parentId, childId) {
            _WebDetachChild.startWebDetachChild(builder);
            _WebDetachChild.addParentId(builder, parentId);
            _WebDetachChild.addChildId(builder, childId);
            return _WebDetachChild.endWebDetachChild(builder);
        }
        unpack() {
            return new WebDetachChildT(this.parentId(), this.childId());
        }
        unpackTo(_o) {
            _o.parentId = this.parentId();
            _o.childId = this.childId();
        }
    };
    var WebDetachChildT = class {
        constructor(parentId = 0, childId = 0){
            this.parentId = parentId;
            this.childId = childId;
        }
        parentId;
        childId;
        pack(builder) {
            return WebDetachChild.createWebDetachChild(builder, this.parentId, this.childId);
        }
    };
    var WebElement = class _WebElement {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebElement(bb, obj) {
            return (obj || new _WebElement).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebElement(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebElement).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        tagId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        customTag(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        className(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startWebElement(builder) {
            builder.startObject(4);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addTagId(builder, tagId) {
            builder.addFieldInt16(1, tagId, 0);
        }
        static addCustomTag(builder, customTagOffset) {
            builder.addFieldOffset(2, customTagOffset, 0);
        }
        static addClassName(builder, classNameOffset) {
            builder.addFieldOffset(3, classNameOffset, 0);
        }
        static endWebElement(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebElement(builder, nodeId, tagId, customTagOffset, classNameOffset) {
            _WebElement.startWebElement(builder);
            _WebElement.addNodeId(builder, nodeId);
            _WebElement.addTagId(builder, tagId);
            _WebElement.addCustomTag(builder, customTagOffset);
            _WebElement.addClassName(builder, classNameOffset);
            return _WebElement.endWebElement(builder);
        }
        unpack() {
            return new WebElementT(this.nodeId(), this.tagId(), this.customTag(), this.className());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.tagId = this.tagId();
            _o.customTag = this.customTag();
            _o.className = this.className();
        }
    };
    var WebElementT = class {
        constructor(nodeId = 0, tagId = 0, customTag = null, className = null){
            this.nodeId = nodeId;
            this.tagId = tagId;
            this.customTag = customTag;
            this.className = className;
        }
        nodeId;
        tagId;
        customTag;
        className;
        pack(builder) {
            const customTag = this.customTag !== null ? builder.createString(this.customTag) : 0;
            const className = this.className !== null ? builder.createString(this.className) : 0;
            return WebElement.createWebElement(builder, this.nodeId, this.tagId, customTag, className);
        }
    };
    var WebProperty = class _WebProperty {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebProperty(bb, obj) {
            return (obj || new _WebProperty).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebProperty(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebProperty).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        nameId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        customName(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        value(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startWebProperty(builder) {
            builder.startObject(4);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addNameId(builder, nameId) {
            builder.addFieldInt16(1, nameId, 0);
        }
        static addCustomName(builder, customNameOffset) {
            builder.addFieldOffset(2, customNameOffset, 0);
        }
        static addValue(builder, valueOffset) {
            builder.addFieldOffset(3, valueOffset, 0);
        }
        static endWebProperty(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebProperty(builder, nodeId, nameId, customNameOffset, valueOffset) {
            _WebProperty.startWebProperty(builder);
            _WebProperty.addNodeId(builder, nodeId);
            _WebProperty.addNameId(builder, nameId);
            _WebProperty.addCustomName(builder, customNameOffset);
            _WebProperty.addValue(builder, valueOffset);
            return _WebProperty.endWebProperty(builder);
        }
        unpack() {
            return new WebPropertyT(this.nodeId(), this.nameId(), this.customName(), this.value());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.nameId = this.nameId();
            _o.customName = this.customName();
            _o.value = this.value();
        }
    };
    var WebPropertyT = class {
        constructor(nodeId = 0, nameId = 0, customName = null, value = null){
            this.nodeId = nodeId;
            this.nameId = nameId;
            this.customName = customName;
            this.value = value;
        }
        nodeId;
        nameId;
        customName;
        value;
        pack(builder) {
            const customName = this.customName !== null ? builder.createString(this.customName) : 0;
            const value = this.value !== null ? builder.createString(this.value) : 0;
            return WebProperty.createWebProperty(builder, this.nodeId, this.nameId, customName, value);
        }
    };
    var WebRegisterNode = class _WebRegisterNode {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebRegisterNode(bb, obj) {
            return (obj || new _WebRegisterNode).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebRegisterNode(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebRegisterNode).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        static startWebRegisterNode(builder) {
            builder.startObject(1);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static endWebRegisterNode(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebRegisterNode(builder, nodeId) {
            _WebRegisterNode.startWebRegisterNode(builder);
            _WebRegisterNode.addNodeId(builder, nodeId);
            return _WebRegisterNode.endWebRegisterNode(builder);
        }
        unpack() {
            return new WebRegisterNodeT(this.nodeId());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
        }
    };
    var WebRegisterNodeT = class {
        constructor(nodeId = 0){
            this.nodeId = nodeId;
        }
        nodeId;
        pack(builder) {
            return WebRegisterNode.createWebRegisterNode(builder, this.nodeId);
        }
    };
    var WebRemoveAttribute = class _WebRemoveAttribute {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebRemoveAttribute(bb, obj) {
            return (obj || new _WebRemoveAttribute).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebRemoveAttribute(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebRemoveAttribute).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        attrId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        customAttr(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startWebRemoveAttribute(builder) {
            builder.startObject(3);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addAttrId(builder, attrId) {
            builder.addFieldInt16(1, attrId, 0);
        }
        static addCustomAttr(builder, customAttrOffset) {
            builder.addFieldOffset(2, customAttrOffset, 0);
        }
        static endWebRemoveAttribute(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebRemoveAttribute(builder, nodeId, attrId, customAttrOffset) {
            _WebRemoveAttribute.startWebRemoveAttribute(builder);
            _WebRemoveAttribute.addNodeId(builder, nodeId);
            _WebRemoveAttribute.addAttrId(builder, attrId);
            _WebRemoveAttribute.addCustomAttr(builder, customAttrOffset);
            return _WebRemoveAttribute.endWebRemoveAttribute(builder);
        }
        unpack() {
            return new WebRemoveAttributeT(this.nodeId(), this.attrId(), this.customAttr());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.attrId = this.attrId();
            _o.customAttr = this.customAttr();
        }
    };
    var WebRemoveAttributeT = class {
        constructor(nodeId = 0, attrId = 0, customAttr = null){
            this.nodeId = nodeId;
            this.attrId = attrId;
            this.customAttr = customAttr;
        }
        nodeId;
        attrId;
        customAttr;
        pack(builder) {
            const customAttr = this.customAttr !== null ? builder.createString(this.customAttr) : 0;
            return WebRemoveAttribute.createWebRemoveAttribute(builder, this.nodeId, this.attrId, customAttr);
        }
    };
    var WebRemoveClass = class _WebRemoveClass {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebRemoveClass(bb, obj) {
            return (obj || new _WebRemoveClass).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebRemoveClass(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebRemoveClass).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        className(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startWebRemoveClass(builder) {
            builder.startObject(2);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addClassName(builder, classNameOffset) {
            builder.addFieldOffset(1, classNameOffset, 0);
        }
        static endWebRemoveClass(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebRemoveClass(builder, nodeId, classNameOffset) {
            _WebRemoveClass.startWebRemoveClass(builder);
            _WebRemoveClass.addNodeId(builder, nodeId);
            _WebRemoveClass.addClassName(builder, classNameOffset);
            return _WebRemoveClass.endWebRemoveClass(builder);
        }
        unpack() {
            return new WebRemoveClassT(this.nodeId(), this.className());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.className = this.className();
        }
    };
    var WebRemoveClassT = class {
        constructor(nodeId = 0, className = null){
            this.nodeId = nodeId;
            this.className = className;
        }
        nodeId;
        className;
        pack(builder) {
            const className = this.className !== null ? builder.createString(this.className) : 0;
            return WebRemoveClass.createWebRemoveClass(builder, this.nodeId, className);
        }
    };
    var WebRemoveListener = class _WebRemoveListener {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebRemoveListener(bb, obj) {
            return (obj || new _WebRemoveListener).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebRemoveListener(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebRemoveListener).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        nameId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        customName(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startWebRemoveListener(builder) {
            builder.startObject(3);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addNameId(builder, nameId) {
            builder.addFieldInt16(1, nameId, 0);
        }
        static addCustomName(builder, customNameOffset) {
            builder.addFieldOffset(2, customNameOffset, 0);
        }
        static endWebRemoveListener(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebRemoveListener(builder, nodeId, nameId, customNameOffset) {
            _WebRemoveListener.startWebRemoveListener(builder);
            _WebRemoveListener.addNodeId(builder, nodeId);
            _WebRemoveListener.addNameId(builder, nameId);
            _WebRemoveListener.addCustomName(builder, customNameOffset);
            return _WebRemoveListener.endWebRemoveListener(builder);
        }
        unpack() {
            return new WebRemoveListenerT(this.nodeId(), this.nameId(), this.customName());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.nameId = this.nameId();
            _o.customName = this.customName();
        }
    };
    var WebRemoveListenerT = class {
        constructor(nodeId = 0, nameId = 0, customName = null){
            this.nodeId = nodeId;
            this.nameId = nameId;
            this.customName = customName;
        }
        nodeId;
        nameId;
        customName;
        pack(builder) {
            const customName = this.customName !== null ? builder.createString(this.customName) : 0;
            return WebRemoveListener.createWebRemoveListener(builder, this.nodeId, this.nameId, customName);
        }
    };
    var WebReplaceNode = class _WebReplaceNode {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebReplaceNode(bb, obj) {
            return (obj || new _WebReplaceNode).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebReplaceNode(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebReplaceNode).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        oldId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        newId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        static startWebReplaceNode(builder) {
            builder.startObject(2);
        }
        static addOldId(builder, oldId) {
            builder.addFieldInt32(0, oldId, 0);
        }
        static addNewId(builder, newId) {
            builder.addFieldInt32(1, newId, 0);
        }
        static endWebReplaceNode(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebReplaceNode(builder, oldId, newId) {
            _WebReplaceNode.startWebReplaceNode(builder);
            _WebReplaceNode.addOldId(builder, oldId);
            _WebReplaceNode.addNewId(builder, newId);
            return _WebReplaceNode.endWebReplaceNode(builder);
        }
        unpack() {
            return new WebReplaceNodeT(this.oldId(), this.newId());
        }
        unpackTo(_o) {
            _o.oldId = this.oldId();
            _o.newId = this.newId();
        }
    };
    var WebReplaceNodeT = class {
        constructor(oldId = 0, newId = 0){
            this.oldId = oldId;
            this.newId = newId;
        }
        oldId;
        newId;
        pack(builder) {
            return WebReplaceNode.createWebReplaceNode(builder, this.oldId, this.newId);
        }
    };
    var WebSetAttribute = class _WebSetAttribute {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebSetAttribute(bb, obj) {
            return (obj || new _WebSetAttribute).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebSetAttribute(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebSetAttribute).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        attrId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        customAttr(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        value(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startWebSetAttribute(builder) {
            builder.startObject(4);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addAttrId(builder, attrId) {
            builder.addFieldInt16(1, attrId, 0);
        }
        static addCustomAttr(builder, customAttrOffset) {
            builder.addFieldOffset(2, customAttrOffset, 0);
        }
        static addValue(builder, valueOffset) {
            builder.addFieldOffset(3, valueOffset, 0);
        }
        static endWebSetAttribute(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebSetAttribute(builder, nodeId, attrId, customAttrOffset, valueOffset) {
            _WebSetAttribute.startWebSetAttribute(builder);
            _WebSetAttribute.addNodeId(builder, nodeId);
            _WebSetAttribute.addAttrId(builder, attrId);
            _WebSetAttribute.addCustomAttr(builder, customAttrOffset);
            _WebSetAttribute.addValue(builder, valueOffset);
            return _WebSetAttribute.endWebSetAttribute(builder);
        }
        unpack() {
            return new WebSetAttributeT(this.nodeId(), this.attrId(), this.customAttr(), this.value());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.attrId = this.attrId();
            _o.customAttr = this.customAttr();
            _o.value = this.value();
        }
    };
    var WebSetAttributeT = class {
        constructor(nodeId = 0, attrId = 0, customAttr = null, value = null){
            this.nodeId = nodeId;
            this.attrId = attrId;
            this.customAttr = customAttr;
            this.value = value;
        }
        nodeId;
        attrId;
        customAttr;
        value;
        pack(builder) {
            const customAttr = this.customAttr !== null ? builder.createString(this.customAttr) : 0;
            const value = this.value !== null ? builder.createString(this.value) : 0;
            return WebSetAttribute.createWebSetAttribute(builder, this.nodeId, this.attrId, customAttr, value);
        }
    };
    var WebStyle = class _WebStyle {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebStyle(bb, obj) {
            return (obj || new _WebStyle).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebStyle(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebStyle).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        nameId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        customName(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        value(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startWebStyle(builder) {
            builder.startObject(4);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addNameId(builder, nameId) {
            builder.addFieldInt16(1, nameId, 0);
        }
        static addCustomName(builder, customNameOffset) {
            builder.addFieldOffset(2, customNameOffset, 0);
        }
        static addValue(builder, valueOffset) {
            builder.addFieldOffset(3, valueOffset, 0);
        }
        static endWebStyle(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebStyle(builder, nodeId, nameId, customNameOffset, valueOffset) {
            _WebStyle.startWebStyle(builder);
            _WebStyle.addNodeId(builder, nodeId);
            _WebStyle.addNameId(builder, nameId);
            _WebStyle.addCustomName(builder, customNameOffset);
            _WebStyle.addValue(builder, valueOffset);
            return _WebStyle.endWebStyle(builder);
        }
        unpack() {
            return new WebStyleT(this.nodeId(), this.nameId(), this.customName(), this.value());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.nameId = this.nameId();
            _o.customName = this.customName();
            _o.value = this.value();
        }
    };
    var WebStyleT = class {
        constructor(nodeId = 0, nameId = 0, customName = null, value = null){
            this.nodeId = nodeId;
            this.nameId = nameId;
            this.customName = customName;
            this.value = value;
        }
        nodeId;
        nameId;
        customName;
        value;
        pack(builder) {
            const customName = this.customName !== null ? builder.createString(this.customName) : 0;
            const value = this.value !== null ? builder.createString(this.value) : 0;
            return WebStyle.createWebStyle(builder, this.nodeId, this.nameId, customName, value);
        }
    };
    var WebText = class _WebText {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebText(bb, obj) {
            return (obj || new _WebText).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebText(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebText).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        content(optionalEncoding) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__string(this.bb_pos + offset, optionalEncoding) : null;
        }
        static startWebText(builder) {
            builder.startObject(2);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static addContent(builder, contentOffset) {
            builder.addFieldOffset(1, contentOffset, 0);
        }
        static endWebText(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebText(builder, nodeId, contentOffset) {
            _WebText.startWebText(builder);
            _WebText.addNodeId(builder, nodeId);
            _WebText.addContent(builder, contentOffset);
            return _WebText.endWebText(builder);
        }
        unpack() {
            return new WebTextT(this.nodeId(), this.content());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
            _o.content = this.content();
        }
    };
    var WebTextT = class {
        constructor(nodeId = 0, content = null){
            this.nodeId = nodeId;
            this.content = content;
        }
        nodeId;
        content;
        pack(builder) {
            const content = this.content !== null ? builder.createString(this.content) : 0;
            return WebText.createWebText(builder, this.nodeId, content);
        }
    };
    var WebUnregisterNode = class _WebUnregisterNode {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsWebUnregisterNode(bb, obj) {
            return (obj || new _WebUnregisterNode).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsWebUnregisterNode(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _WebUnregisterNode).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        nodeId() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        static startWebUnregisterNode(builder) {
            builder.startObject(1);
        }
        static addNodeId(builder, nodeId) {
            builder.addFieldInt32(0, nodeId, 0);
        }
        static endWebUnregisterNode(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createWebUnregisterNode(builder, nodeId) {
            _WebUnregisterNode.startWebUnregisterNode(builder);
            _WebUnregisterNode.addNodeId(builder, nodeId);
            return _WebUnregisterNode.endWebUnregisterNode(builder);
        }
        unpack() {
            return new WebUnregisterNodeT(this.nodeId());
        }
        unpackTo(_o) {
            _o.nodeId = this.nodeId();
        }
    };
    var WebUnregisterNodeT = class {
        constructor(nodeId = 0){
            this.nodeId = nodeId;
        }
        nodeId;
        pack(builder) {
            return WebUnregisterNode.createWebUnregisterNode(builder, this.nodeId);
        }
    };
    var WebOperation = ((WebOperation2)=>{
        WebOperation2[WebOperation2["NONE"] = 0] = "NONE";
        WebOperation2[WebOperation2["WebElement"] = 1] = "WebElement";
        WebOperation2[WebOperation2["WebText"] = 2] = "WebText";
        WebOperation2[WebOperation2["WebSetAttribute"] = 3] = "WebSetAttribute";
        WebOperation2[WebOperation2["WebRemoveAttribute"] = 4] = "WebRemoveAttribute";
        WebOperation2[WebOperation2["WebProperty"] = 5] = "WebProperty";
        WebOperation2[WebOperation2["WebAddListener"] = 6] = "WebAddListener";
        WebOperation2[WebOperation2["WebRemoveListener"] = 7] = "WebRemoveListener";
        WebOperation2[WebOperation2["WebReplaceNode"] = 8] = "WebReplaceNode";
        WebOperation2[WebOperation2["WebStyle"] = 9] = "WebStyle";
        WebOperation2[WebOperation2["WebAddClass"] = 10] = "WebAddClass";
        WebOperation2[WebOperation2["WebRemoveClass"] = 11] = "WebRemoveClass";
        WebOperation2[WebOperation2["HtmlMorph"] = 12] = "HtmlMorph";
        WebOperation2[WebOperation2["WebRegisterNode"] = 13] = "WebRegisterNode";
        WebOperation2[WebOperation2["WebUnregisterNode"] = 14] = "WebUnregisterNode";
        WebOperation2[WebOperation2["ScriptNode"] = 15] = "ScriptNode";
        WebOperation2[WebOperation2["WebBehaviour"] = 16] = "WebBehaviour";
        WebOperation2[WebOperation2["WebDetachChild"] = 17] = "WebDetachChild";
        return WebOperation2;
    })(WebOperation || {});
    function unionToWebOperation(type, accessor) {
        switch(WebOperation[type]){
            case "NONE":
                return null;
            case "WebElement":
                return accessor(new WebElement);
            case "WebText":
                return accessor(new WebText);
            case "WebSetAttribute":
                return accessor(new WebSetAttribute);
            case "WebRemoveAttribute":
                return accessor(new WebRemoveAttribute);
            case "WebProperty":
                return accessor(new WebProperty);
            case "WebAddListener":
                return accessor(new WebAddListener);
            case "WebRemoveListener":
                return accessor(new WebRemoveListener);
            case "WebReplaceNode":
                return accessor(new WebReplaceNode);
            case "WebStyle":
                return accessor(new WebStyle);
            case "WebAddClass":
                return accessor(new WebAddClass);
            case "WebRemoveClass":
                return accessor(new WebRemoveClass);
            case "HtmlMorph":
                return accessor(new HtmlMorph);
            case "WebRegisterNode":
                return accessor(new WebRegisterNode);
            case "WebUnregisterNode":
                return accessor(new WebUnregisterNode);
            case "ScriptNode":
                return accessor(new ScriptNode);
            case "WebBehaviour":
                return accessor(new WebBehaviour);
            case "WebDetachChild":
                return accessor(new WebDetachChild);
            default:
                return null;
        }
    }
    function unionListToWebOperation(type, accessor, index) {
        switch(WebOperation[type]){
            case "NONE":
                return null;
            case "WebElement":
                return accessor(index, new WebElement);
            case "WebText":
                return accessor(index, new WebText);
            case "WebSetAttribute":
                return accessor(index, new WebSetAttribute);
            case "WebRemoveAttribute":
                return accessor(index, new WebRemoveAttribute);
            case "WebProperty":
                return accessor(index, new WebProperty);
            case "WebAddListener":
                return accessor(index, new WebAddListener);
            case "WebRemoveListener":
                return accessor(index, new WebRemoveListener);
            case "WebReplaceNode":
                return accessor(index, new WebReplaceNode);
            case "WebStyle":
                return accessor(index, new WebStyle);
            case "WebAddClass":
                return accessor(index, new WebAddClass);
            case "WebRemoveClass":
                return accessor(index, new WebRemoveClass);
            case "HtmlMorph":
                return accessor(index, new HtmlMorph);
            case "WebRegisterNode":
                return accessor(index, new WebRegisterNode);
            case "WebUnregisterNode":
                return accessor(index, new WebUnregisterNode);
            case "ScriptNode":
                return accessor(index, new ScriptNode);
            case "WebBehaviour":
                return accessor(index, new WebBehaviour);
            case "WebDetachChild":
                return accessor(index, new WebDetachChild);
            default:
                return null;
        }
    }
    var Operation = class _Operation {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsOperation(bb, obj) {
            return (obj || new _Operation).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsOperation(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _Operation).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        uiType() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        ui(obj) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__union(obj, this.bb_pos + offset) : null;
        }
        webType() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        web(obj) {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.__union(obj, this.bb_pos + offset) : null;
        }
        mobileType() {
            const offset = this.bb.__offset(this.bb_pos, 12);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        mobile(obj) {
            const offset = this.bb.__offset(this.bb_pos, 14);
            return offset ? this.bb.__union(obj, this.bb_pos + offset) : null;
        }
        columnar(obj) {
            const offset = this.bb.__offset(this.bb_pos, 16);
            return offset ? (obj || new ColumnarBatch).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        static startOperation(builder) {
            builder.startObject(7);
        }
        static addUiType(builder, uiType) {
            builder.addFieldInt8(0, uiType, 0);
        }
        static addUi(builder, uiOffset) {
            builder.addFieldOffset(1, uiOffset, 0);
        }
        static addWebType(builder, webType) {
            builder.addFieldInt8(2, webType, 0);
        }
        static addWeb(builder, webOffset) {
            builder.addFieldOffset(3, webOffset, 0);
        }
        static addMobileType(builder, mobileType) {
            builder.addFieldInt8(4, mobileType, 0);
        }
        static addMobile(builder, mobileOffset) {
            builder.addFieldOffset(5, mobileOffset, 0);
        }
        static addColumnar(builder, columnarOffset) {
            builder.addFieldOffset(6, columnarOffset, 0);
        }
        static endOperation(builder) {
            const offset = builder.endObject();
            return offset;
        }
        unpack() {
            return new OperationT3(this.uiType(), (()=>{
                const temp = unionToUiOperation(this.uiType(), this.ui.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })(), this.webType(), (()=>{
                const temp = unionToWebOperation(this.webType(), this.web.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })(), this.mobileType(), (()=>{
                const temp = unionToMobileOperation(this.mobileType(), this.mobile.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })(), this.columnar() !== null ? this.columnar().unpack() : null);
        }
        unpackTo(_o) {
            _o.uiType = this.uiType();
            _o.ui = (()=>{
                const temp = unionToUiOperation(this.uiType(), this.ui.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })();
            _o.webType = this.webType();
            _o.web = (()=>{
                const temp = unionToWebOperation(this.webType(), this.web.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })();
            _o.mobileType = this.mobileType();
            _o.mobile = (()=>{
                const temp = unionToMobileOperation(this.mobileType(), this.mobile.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })();
            _o.columnar = this.columnar() !== null ? this.columnar().unpack() : null;
        }
    };
    var OperationT3 = class {
        constructor(uiType = 0, ui = null, webType = 0, web = null, mobileType = 0, mobile = null, columnar = null){
            this.uiType = uiType;
            this.ui = ui;
            this.webType = webType;
            this.web = web;
            this.mobileType = mobileType;
            this.mobile = mobile;
            this.columnar = columnar;
        }
        uiType;
        ui;
        webType;
        web;
        mobileType;
        mobile;
        columnar;
        pack(builder) {
            const ui = builder.createObjectOffset(this.ui);
            const web = builder.createObjectOffset(this.web);
            const mobile = builder.createObjectOffset(this.mobile);
            const columnar = this.columnar !== null ? this.columnar.pack(builder) : 0;
            Operation.startOperation(builder);
            Operation.addUiType(builder, this.uiType);
            Operation.addUi(builder, ui);
            Operation.addWebType(builder, this.webType);
            Operation.addWeb(builder, web);
            Operation.addMobileType(builder, this.mobileType);
            Operation.addMobile(builder, mobile);
            Operation.addColumnar(builder, columnar);
            return Operation.endOperation(builder);
        }
    };
    var Message = class _Message {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsMessage(bb, obj) {
            return (obj || new _Message).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsMessage(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _Message).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        operation(obj) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? (obj || new Operation).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        data(obj) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? (obj || new Data).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        control(obj) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? (obj || new Control).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        static startMessage(builder) {
            builder.startObject(3);
        }
        static addOperation(builder, operationOffset) {
            builder.addFieldOffset(0, operationOffset, 0);
        }
        static addData(builder, dataOffset) {
            builder.addFieldOffset(1, dataOffset, 0);
        }
        static addControl(builder, controlOffset) {
            builder.addFieldOffset(2, controlOffset, 0);
        }
        static endMessage(builder) {
            const offset = builder.endObject();
            return offset;
        }
        unpack() {
            return new MessageT(this.operation() !== null ? this.operation().unpack() : null, this.data() !== null ? this.data().unpack() : null, this.control() !== null ? this.control().unpack() : null);
        }
        unpackTo(_o) {
            _o.operation = this.operation() !== null ? this.operation().unpack() : null;
            _o.data = this.data() !== null ? this.data().unpack() : null;
            _o.control = this.control() !== null ? this.control().unpack() : null;
        }
    };
    var MessageT = class {
        constructor(operation = null, data = null, control = null){
            this.operation = operation;
            this.data = data;
            this.control = control;
        }
        operation;
        data;
        control;
        pack(builder) {
            const operation = this.operation !== null ? this.operation.pack(builder) : 0;
            const data = this.data !== null ? this.data.pack(builder) : 0;
            const control = this.control !== null ? this.control.pack(builder) : 0;
            Message.startMessage(builder);
            Message.addOperation(builder, operation);
            Message.addData(builder, data);
            Message.addControl(builder, control);
            return Message.endMessage(builder);
        }
    };
    var FrameMessage = class _FrameMessage {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsFrameMessage(bb, obj) {
            return (obj || new _FrameMessage).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsFrameMessage(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _FrameMessage).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        ack() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        messages(index, obj) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? (obj || new Message).__init(this.bb.__indirect(this.bb.__vector(this.bb_pos + offset) + index * 4), this.bb) : null;
        }
        messagesLength() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        subscriptionId() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        static startFrameMessage(builder) {
            builder.startObject(3);
        }
        static addAck(builder, ack) {
            builder.addFieldInt64(0, ack, BigInt("0"));
        }
        static addMessages(builder, messagesOffset) {
            builder.addFieldOffset(1, messagesOffset, 0);
        }
        static createMessagesVector(builder, data) {
            builder.startVector(4, data.length, 4);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addOffset(data[i]);
            }
            return builder.endVector();
        }
        static startMessagesVector(builder, numElems) {
            builder.startVector(4, numElems, 4);
        }
        static addSubscriptionId(builder, subscriptionId) {
            builder.addFieldInt64(2, subscriptionId, BigInt("0"));
        }
        static endFrameMessage(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createFrameMessage(builder, ack, messagesOffset, subscriptionId) {
            _FrameMessage.startFrameMessage(builder);
            _FrameMessage.addAck(builder, ack);
            _FrameMessage.addMessages(builder, messagesOffset);
            _FrameMessage.addSubscriptionId(builder, subscriptionId);
            return _FrameMessage.endFrameMessage(builder);
        }
        unpack() {
            return new FrameMessageT(this.ack(), this.bb.createObjList(this.messages.bind(this), this.messagesLength()), this.subscriptionId());
        }
        unpackTo(_o) {
            _o.ack = this.ack();
            _o.messages = this.bb.createObjList(this.messages.bind(this), this.messagesLength());
            _o.subscriptionId = this.subscriptionId();
        }
    };
    var FrameMessageT = class {
        constructor(ack = BigInt("0"), messages = [], subscriptionId = BigInt("0")){
            this.ack = ack;
            this.messages = messages;
            this.subscriptionId = subscriptionId;
        }
        ack;
        messages;
        subscriptionId;
        pack(builder) {
            const messages = FrameMessage.createMessagesVector(builder, builder.createObjectOffsetList(this.messages));
            return FrameMessage.createFrameMessage(builder, this.ack, messages, this.subscriptionId);
        }
    };
    var ResumeSubscription = class _ResumeSubscription {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsResumeSubscription(bb, obj) {
            return (obj || new _ResumeSubscription).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsResumeSubscription(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _ResumeSubscription).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        deliverySession() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        subscriptionId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        streamId() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        after() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        static startResumeSubscription(builder) {
            builder.startObject(4);
        }
        static addDeliverySession(builder, deliverySession) {
            builder.addFieldInt64(0, deliverySession, BigInt("0"));
        }
        static addSubscriptionId(builder, subscriptionId) {
            builder.addFieldInt64(1, subscriptionId, BigInt("0"));
        }
        static addStreamId(builder, streamId) {
            builder.addFieldInt64(2, streamId, BigInt("0"));
        }
        static addAfter(builder, after) {
            builder.addFieldInt64(3, after, BigInt("0"));
        }
        static endResumeSubscription(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static createResumeSubscription(builder, deliverySession, subscriptionId, streamId, after) {
            _ResumeSubscription.startResumeSubscription(builder);
            _ResumeSubscription.addDeliverySession(builder, deliverySession);
            _ResumeSubscription.addSubscriptionId(builder, subscriptionId);
            _ResumeSubscription.addStreamId(builder, streamId);
            _ResumeSubscription.addAfter(builder, after);
            return _ResumeSubscription.endResumeSubscription(builder);
        }
        unpack() {
            return new ResumeSubscriptionT(this.deliverySession(), this.subscriptionId(), this.streamId(), this.after());
        }
        unpackTo(_o) {
            _o.deliverySession = this.deliverySession();
            _o.subscriptionId = this.subscriptionId();
            _o.streamId = this.streamId();
            _o.after = this.after();
        }
    };
    var ResumeSubscriptionT = class {
        constructor(deliverySession = BigInt("0"), subscriptionId = BigInt("0"), streamId = BigInt("0"), after = BigInt("0")){
            this.deliverySession = deliverySession;
            this.subscriptionId = subscriptionId;
            this.streamId = streamId;
            this.after = after;
        }
        deliverySession;
        subscriptionId;
        streamId;
        after;
        pack(builder) {
            return ResumeSubscription.createResumeSubscription(builder, this.deliverySession, this.subscriptionId, this.streamId, this.after);
        }
    };
    var FrameRequest = class _FrameRequest {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsFrameRequest(bb, obj) {
            return (obj || new _FrameRequest).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsFrameRequest(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _FrameRequest).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        action(obj) {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? (obj || new ActionRequest).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        hello(obj) {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? (obj || new ClientHello).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        resume(obj) {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? (obj || new ResumeSubscription).__init(this.bb.__indirect(this.bb_pos + offset), this.bb) : null;
        }
        static startFrameRequest(builder) {
            builder.startObject(3);
        }
        static addAction(builder, actionOffset) {
            builder.addFieldOffset(0, actionOffset, 0);
        }
        static addHello(builder, helloOffset) {
            builder.addFieldOffset(1, helloOffset, 0);
        }
        static addResume(builder, resumeOffset) {
            builder.addFieldOffset(2, resumeOffset, 0);
        }
        static endFrameRequest(builder) {
            const offset = builder.endObject();
            return offset;
        }
        unpack() {
            return new FrameRequestT(this.action() !== null ? this.action().unpack() : null, this.hello() !== null ? this.hello().unpack() : null, this.resume() !== null ? this.resume().unpack() : null);
        }
        unpackTo(_o) {
            _o.action = this.action() !== null ? this.action().unpack() : null;
            _o.hello = this.hello() !== null ? this.hello().unpack() : null;
            _o.resume = this.resume() !== null ? this.resume().unpack() : null;
        }
    };
    var FrameRequestT = class {
        constructor(action = null, hello = null, resume = null){
            this.action = action;
            this.hello = hello;
            this.resume = resume;
        }
        action;
        hello;
        resume;
        pack(builder) {
            const action = this.action !== null ? this.action.pack(builder) : 0;
            const hello = this.hello !== null ? this.hello.pack(builder) : 0;
            const resume = this.resume !== null ? this.resume.pack(builder) : 0;
            FrameRequest.startFrameRequest(builder);
            FrameRequest.addAction(builder, action);
            FrameRequest.addHello(builder, hello);
            FrameRequest.addResume(builder, resume);
            return FrameRequest.endFrameRequest(builder);
        }
    };
    var FrameKind = ((FrameKind2)=>{
        FrameKind2[FrameKind2["NONE"] = 0] = "NONE";
        FrameKind2[FrameKind2["FrameRequest"] = 1] = "FrameRequest";
        FrameKind2[FrameKind2["FrameMessage"] = 2] = "FrameMessage";
        return FrameKind2;
    })(FrameKind || {});
    function unionToFrameKind(type, accessor) {
        switch(FrameKind[type]){
            case "NONE":
                return null;
            case "FrameRequest":
                return accessor(new FrameRequest);
            case "FrameMessage":
                return accessor(new FrameMessage);
            default:
                return null;
        }
    }
    function unionListToFrameKind(type, accessor, index) {
        switch(FrameKind[type]){
            case "NONE":
                return null;
            case "FrameRequest":
                return accessor(index, new FrameRequest);
            case "FrameMessage":
                return accessor(index, new FrameMessage);
            default:
                return null;
        }
    }
    var Frame = class _Frame {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsFrame(bb, obj) {
            return (obj || new _Frame).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsFrame(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _Frame).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static bufferHasIdentifier(bb) {
            return bb.__has_identifier("EWF1");
        }
        schemaVersion() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        streamId() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        sequenceId() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        kindType() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        kind(obj) {
            const offset = this.bb.__offset(this.bb_pos, 12);
            return offset ? this.bb.__union(obj, this.bb_pos + offset) : null;
        }
        static startFrame(builder) {
            builder.startObject(5);
        }
        static addSchemaVersion(builder, schemaVersion) {
            builder.addFieldInt16(0, schemaVersion, 0);
        }
        static addStreamId(builder, streamId) {
            builder.addFieldInt64(1, streamId, BigInt("0"));
        }
        static addSequenceId(builder, sequenceId) {
            builder.addFieldInt64(2, sequenceId, BigInt("0"));
        }
        static addKindType(builder, kindType) {
            builder.addFieldInt8(3, kindType, 0);
        }
        static addKind(builder, kindOffset) {
            builder.addFieldOffset(4, kindOffset, 0);
        }
        static endFrame(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static finishFrameBuffer(builder, offset) {
            builder.finish(offset, "EWF1");
        }
        static finishSizePrefixedFrameBuffer(builder, offset) {
            builder.finish(offset, "EWF1", true);
        }
        static createFrame(builder, schemaVersion, streamId, sequenceId, kindType, kindOffset) {
            _Frame.startFrame(builder);
            _Frame.addSchemaVersion(builder, schemaVersion);
            _Frame.addStreamId(builder, streamId);
            _Frame.addSequenceId(builder, sequenceId);
            _Frame.addKindType(builder, kindType);
            _Frame.addKind(builder, kindOffset);
            return _Frame.endFrame(builder);
        }
        unpack() {
            return new FrameT(this.schemaVersion(), this.streamId(), this.sequenceId(), this.kindType(), (()=>{
                const temp = unionToFrameKind(this.kindType(), this.kind.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })());
        }
        unpackTo(_o) {
            _o.schemaVersion = this.schemaVersion();
            _o.streamId = this.streamId();
            _o.sequenceId = this.sequenceId();
            _o.kindType = this.kindType();
            _o.kind = (()=>{
                const temp = unionToFrameKind(this.kindType(), this.kind.bind(this));
                if (temp === null) {
                    return null;
                }
                return temp.unpack();
            })();
        }
    };
    var FrameT = class {
        constructor(schemaVersion = 0, streamId = BigInt("0"), sequenceId = BigInt("0"), kindType = 0, kind = null){
            this.schemaVersion = schemaVersion;
            this.streamId = streamId;
            this.sequenceId = sequenceId;
            this.kindType = kindType;
            this.kind = kind;
        }
        schemaVersion;
        streamId;
        sequenceId;
        kindType;
        kind;
        pack(builder) {
            const kind = builder.createObjectOffset(this.kind);
            return Frame.createFrame(builder, this.schemaVersion, this.streamId, this.sequenceId, this.kindType, kind);
        }
    };
    var SignedState = class _SignedState {
        bb = null;
        bb_pos = 0;
        __init(i, bb) {
            this.bb_pos = i;
            this.bb = bb;
            return this;
        }
        static getRootAsSignedState(bb, obj) {
            return (obj || new _SignedState).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static getSizePrefixedRootAsSignedState(bb, obj) {
            bb.setPosition(bb.position() + SIZE_PREFIX_LENGTH);
            return (obj || new _SignedState).__init(bb.readInt32(bb.position()) + bb.position(), bb);
        }
        static bufferHasIdentifier(bb) {
            return bb.__has_identifier("EWS1");
        }
        formatVersion() {
            const offset = this.bb.__offset(this.bb_pos, 4);
            return offset ? this.bb.readUint16(this.bb_pos + offset) : 0;
        }
        flags() {
            const offset = this.bb.__offset(this.bb_pos, 6);
            return offset ? this.bb.readUint8(this.bb_pos + offset) : 0;
        }
        keyId() {
            const offset = this.bb.__offset(this.bb_pos, 8);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        issuedAt() {
            const offset = this.bb.__offset(this.bb_pos, 10);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        expiresAt() {
            const offset = this.bb.__offset(this.bb_pos, 12);
            return offset ? this.bb.readUint64(this.bb_pos + offset) : BigInt("0");
        }
        componentType() {
            const offset = this.bb.__offset(this.bb_pos, 14);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        rootNode() {
            const offset = this.bb.__offset(this.bb_pos, 16);
            return offset ? this.bb.readUint32(this.bb_pos + offset) : 0;
        }
        sessionBinding(index) {
            const offset = this.bb.__offset(this.bb_pos, 18);
            return offset ? this.bb.readUint8(this.bb.__vector(this.bb_pos + offset) + index) : 0;
        }
        sessionBindingLength() {
            const offset = this.bb.__offset(this.bb_pos, 18);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        sessionBindingArray() {
            const offset = this.bb.__offset(this.bb_pos, 18);
            return offset ? new Uint8Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        body(index) {
            const offset = this.bb.__offset(this.bb_pos, 20);
            return offset ? this.bb.readUint8(this.bb.__vector(this.bb_pos + offset) + index) : 0;
        }
        bodyLength() {
            const offset = this.bb.__offset(this.bb_pos, 20);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        bodyArray() {
            const offset = this.bb.__offset(this.bb_pos, 20);
            return offset ? new Uint8Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        tag(index) {
            const offset = this.bb.__offset(this.bb_pos, 22);
            return offset ? this.bb.readUint8(this.bb.__vector(this.bb_pos + offset) + index) : 0;
        }
        tagLength() {
            const offset = this.bb.__offset(this.bb_pos, 22);
            return offset ? this.bb.__vector_len(this.bb_pos + offset) : 0;
        }
        tagArray() {
            const offset = this.bb.__offset(this.bb_pos, 22);
            return offset ? new Uint8Array(this.bb.bytes().buffer, this.bb.bytes().byteOffset + this.bb.__vector(this.bb_pos + offset), this.bb.__vector_len(this.bb_pos + offset)) : null;
        }
        static startSignedState(builder) {
            builder.startObject(10);
        }
        static addFormatVersion(builder, formatVersion) {
            builder.addFieldInt16(0, formatVersion, 0);
        }
        static addFlags(builder, flags) {
            builder.addFieldInt8(1, flags, 0);
        }
        static addKeyId(builder, keyId) {
            builder.addFieldInt32(2, keyId, 0);
        }
        static addIssuedAt(builder, issuedAt) {
            builder.addFieldInt64(3, issuedAt, BigInt("0"));
        }
        static addExpiresAt(builder, expiresAt) {
            builder.addFieldInt64(4, expiresAt, BigInt("0"));
        }
        static addComponentType(builder, componentType) {
            builder.addFieldInt32(5, componentType, 0);
        }
        static addRootNode(builder, rootNode) {
            builder.addFieldInt32(6, rootNode, 0);
        }
        static addSessionBinding(builder, sessionBindingOffset) {
            builder.addFieldOffset(7, sessionBindingOffset, 0);
        }
        static createSessionBindingVector(builder, data) {
            builder.startVector(1, data.length, 1);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt8(data[i]);
            }
            return builder.endVector();
        }
        static startSessionBindingVector(builder, numElems) {
            builder.startVector(1, numElems, 1);
        }
        static addBody(builder, bodyOffset) {
            builder.addFieldOffset(8, bodyOffset, 0);
        }
        static createBodyVector(builder, data) {
            builder.startVector(1, data.length, 1);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt8(data[i]);
            }
            return builder.endVector();
        }
        static startBodyVector(builder, numElems) {
            builder.startVector(1, numElems, 1);
        }
        static addTag(builder, tagOffset) {
            builder.addFieldOffset(9, tagOffset, 0);
        }
        static createTagVector(builder, data) {
            builder.startVector(1, data.length, 1);
            for(let i = data.length - 1; i >= 0; i--){
                builder.addInt8(data[i]);
            }
            return builder.endVector();
        }
        static startTagVector(builder, numElems) {
            builder.startVector(1, numElems, 1);
        }
        static endSignedState(builder) {
            const offset = builder.endObject();
            return offset;
        }
        static finishSignedStateBuffer(builder, offset) {
            builder.finish(offset, "EWS1");
        }
        static finishSizePrefixedSignedStateBuffer(builder, offset) {
            builder.finish(offset, "EWS1", true);
        }
        static createSignedState(builder, formatVersion, flags, keyId, issuedAt, expiresAt, componentType, rootNode, sessionBindingOffset, bodyOffset, tagOffset) {
            _SignedState.startSignedState(builder);
            _SignedState.addFormatVersion(builder, formatVersion);
            _SignedState.addFlags(builder, flags);
            _SignedState.addKeyId(builder, keyId);
            _SignedState.addIssuedAt(builder, issuedAt);
            _SignedState.addExpiresAt(builder, expiresAt);
            _SignedState.addComponentType(builder, componentType);
            _SignedState.addRootNode(builder, rootNode);
            _SignedState.addSessionBinding(builder, sessionBindingOffset);
            _SignedState.addBody(builder, bodyOffset);
            _SignedState.addTag(builder, tagOffset);
            return _SignedState.endSignedState(builder);
        }
        unpack() {
            return new SignedStateT(this.formatVersion(), this.flags(), this.keyId(), this.issuedAt(), this.expiresAt(), this.componentType(), this.rootNode(), this.bb.createScalarList(this.sessionBinding.bind(this), this.sessionBindingLength()), this.bb.createScalarList(this.body.bind(this), this.bodyLength()), this.bb.createScalarList(this.tag.bind(this), this.tagLength()));
        }
        unpackTo(_o) {
            _o.formatVersion = this.formatVersion();
            _o.flags = this.flags();
            _o.keyId = this.keyId();
            _o.issuedAt = this.issuedAt();
            _o.expiresAt = this.expiresAt();
            _o.componentType = this.componentType();
            _o.rootNode = this.rootNode();
            _o.sessionBinding = this.bb.createScalarList(this.sessionBinding.bind(this), this.sessionBindingLength());
            _o.body = this.bb.createScalarList(this.body.bind(this), this.bodyLength());
            _o.tag = this.bb.createScalarList(this.tag.bind(this), this.tagLength());
        }
    };
    var SignedStateT = class {
        constructor(formatVersion = 0, flags = 0, keyId = 0, issuedAt = BigInt("0"), expiresAt = BigInt("0"), componentType = 0, rootNode = 0, sessionBinding = [], body = [], tag = []){
            this.formatVersion = formatVersion;
            this.flags = flags;
            this.keyId = keyId;
            this.issuedAt = issuedAt;
            this.expiresAt = expiresAt;
            this.componentType = componentType;
            this.rootNode = rootNode;
            this.sessionBinding = sessionBinding;
            this.body = body;
            this.tag = tag;
        }
        formatVersion;
        flags;
        keyId;
        issuedAt;
        expiresAt;
        componentType;
        rootNode;
        sessionBinding;
        body;
        tag;
        pack(builder) {
            const sessionBinding = SignedState.createSessionBindingVector(builder, this.sessionBinding);
            const body = SignedState.createBodyVector(builder, this.body);
            const tag = SignedState.createTagVector(builder, this.tag);
            return SignedState.createSignedState(builder, this.formatVersion, this.flags, this.keyId, this.issuedAt, this.expiresAt, this.componentType, this.rootNode, sessionBinding, body, tag);
        }
    };
    const mod1 = {
        ActionError: ActionError,
        ActionErrorT: ActionErrorT,
        ActionInput: ActionInput,
        ActionInputT: ActionInputT,
        ActionRequest: ActionRequest,
        ActionRequestT: ActionRequestT,
        Alignment: Alignment,
        AlignmentValue: AlignmentValue,
        AlignmentValueT: AlignmentValueT,
        AppendChild: AppendChild,
        AppendChildT: AppendChildT,
        ArrowIpcBlob: ArrowIpcBlob,
        ArrowIpcBlobT: ArrowIpcBlobT,
        AttachState: AttachState,
        AttachStateT: AttachStateT,
        Axis: Axis,
        AxisValue: AxisValue,
        AxisValueT: AxisValueT,
        BehaviourArg: BehaviourArg,
        BehaviourArgT: BehaviourArgT,
        BehaviourId: BehaviourId,
        BindAction: BindAction,
        BindActionT: BindActionT,
        BoolValue: BoolValue,
        BoolValueT: BoolValueT,
        Bools: Bools,
        BoolsT: BoolsT,
        Builder: Builder,
        ByteBuffer: ByteBuffer,
        CapsStatus: CapsStatus,
        ClientHello: ClientHello,
        ClientHelloT: ClientHelloT,
        ColorToken: ColorToken,
        ColorTokenT: ColorTokenT,
        Column: Column,
        ColumnData: ColumnData,
        ColumnT: ColumnT,
        ColumnarBatch: ColumnarBatch,
        ColumnarBatchT: ColumnarBatchT,
        ConnectionSharing: ConnectionSharing,
        Control: Control,
        ControlT: ControlT,
        CreateNode: CreateNode,
        CreateNodeT: CreateNodeT,
        Data: Data,
        DataT: DataT,
        DataTable: DataTable,
        DataTableT: DataTableT,
        ErrorCode: ErrorCode,
        EventId: EventId,
        F64s: F64s,
        F64sT: F64sT,
        FontToken: FontToken,
        FontTokenT: FontTokenT,
        Frame: Frame,
        FrameKind: FrameKind,
        FrameMessage: FrameMessage,
        FrameMessageT: FrameMessageT,
        FrameRequest: FrameRequest,
        FrameRequestT: FrameRequestT,
        FrameT: FrameT,
        HtmlMorph: HtmlMorph,
        HtmlMorphT: HtmlMorphT,
        I32s: I32s,
        I32sT: I32sT,
        I64s: I64s,
        I64sT: I64sT,
        InsertBefore: InsertBefore,
        InsertBeforeT: InsertBeforeT,
        Length: Length,
        LengthT: LengthT,
        LengthUnit: LengthUnit,
        LengthValue: LengthValue,
        LengthValueT: LengthValueT,
        LiveKind: LiveKind,
        LiveMount: LiveMount,
        LiveMountT: LiveMountT,
        LiveTransport: LiveTransport,
        Message: Message,
        MessagePackBlob: MessagePackBlob,
        MessagePackBlobT: MessagePackBlobT,
        MessageT: MessageT,
        MobileBehaviour: MobileBehaviour,
        MobileBehaviourId: MobileBehaviourId,
        MobileBehaviourT: MobileBehaviourT,
        MobileOperation: MobileOperation,
        MorphAction: MorphAction,
        MountSurface: MountSurface,
        MountWebView: MountWebView,
        MountWebViewT: MountWebViewT,
        NativeArg: NativeArg,
        NativeArgT: NativeArgT,
        NativeCommand: NativeCommand,
        NativeCommandId: NativeCommandId,
        NativeCommandT: NativeCommandT,
        NodeKind: NodeKind,
        Operation: Operation,
        OperationT: OperationT3,
        Place: Place,
        PlaceT: PlaceT,
        PlacementKind: PlacementKind,
        Platform: Platform,
        PropId: PropId,
        PropValue: PropValue,
        Remove: Remove,
        RemoveT: RemoveT,
        ReplaceSubtree: ReplaceSubtree,
        ReplaceSubtreeT: ReplaceSubtreeT,
        ReplayExpired: ReplayExpired,
        ReplayExpiredT: ReplayExpiredT,
        ResumeSubscription: ResumeSubscription,
        ResumeSubscriptionT: ResumeSubscriptionT,
        ScriptNode: ScriptNode,
        ScriptNodeT: ScriptNodeT,
        SelectorKind: SelectorKind,
        ServerAction: ServerAction,
        ServerActionT: ServerActionT,
        ServerCaps: ServerCaps,
        ServerCapsT: ServerCapsT,
        SetProp: SetProp,
        SetPropT: SetPropT,
        SetText: SetText,
        SetTextT: SetTextT,
        SignedState: SignedState,
        SignedStateT: SignedStateT,
        StateExpired: StateExpired,
        StateExpiredT: StateExpiredT,
        Strings: Strings,
        StringsT: StringsT,
        TargetSelector: TargetSelector,
        TargetSelectorT: TargetSelectorT,
        TextValue: TextValue,
        TextValueT: TextValueT,
        UiBehaviour: UiBehaviour,
        UiBehaviourT: UiBehaviourT,
        UiOperation: UiOperation,
        UnbindAction: UnbindAction,
        UnbindActionT: UnbindActionT,
        UpdateRequired: UpdateRequired,
        UpdateRequiredT: UpdateRequiredT,
        WebAddClass: WebAddClass,
        WebAddClassT: WebAddClassT,
        WebAddListener: WebAddListener,
        WebAddListenerT: WebAddListenerT,
        WebBehaviour: WebBehaviour,
        WebBehaviourId: WebBehaviourId,
        WebBehaviourT: WebBehaviourT,
        WebDetachChild: WebDetachChild,
        WebDetachChildT: WebDetachChildT,
        WebElement: WebElement,
        WebElementT: WebElementT,
        WebOperation: WebOperation,
        WebProperty: WebProperty,
        WebPropertyT: WebPropertyT,
        WebRegisterNode: WebRegisterNode,
        WebRegisterNodeT: WebRegisterNodeT,
        WebRemoveAttribute: WebRemoveAttribute,
        WebRemoveAttributeT: WebRemoveAttributeT,
        WebRemoveClass: WebRemoveClass,
        WebRemoveClassT: WebRemoveClassT,
        WebRemoveListener: WebRemoveListener,
        WebRemoveListenerT: WebRemoveListenerT,
        WebReplaceNode: WebReplaceNode,
        WebReplaceNodeT: WebReplaceNodeT,
        WebSetAttribute: WebSetAttribute,
        WebSetAttributeT: WebSetAttributeT,
        WebStyle: WebStyle,
        WebStyleT: WebStyleT,
        WebText: WebText,
        WebTextT: WebTextT,
        WebUnregisterNode: WebUnregisterNode,
        WebUnregisterNodeT: WebUnregisterNodeT,
        unionListToColumnData: unionListToColumnData,
        unionListToFrameKind: unionListToFrameKind,
        unionListToMobileOperation: unionListToMobileOperation,
        unionListToPropValue: unionListToPropValue,
        unionListToUiOperation: unionListToUiOperation,
        unionListToWebOperation: unionListToWebOperation,
        unionToColumnData: unionToColumnData,
        unionToFrameKind: unionToFrameKind,
        unionToMobileOperation: unionToMobileOperation,
        unionToPropValue: unionToPropValue,
        unionToUiOperation: unionToUiOperation,
        unionToWebOperation: unionToWebOperation
    };
    const VOCABULARY_VERSION = 12;
    const PROP_KIND = Object.freeze([
        null,
        "Length",
        "Length",
        "Alignment",
        "ColorToken",
        "FontToken",
        "Bool",
        "Text",
        "Axis",
        "Text",
        "Bool",
        "Text",
        "Text",
        "ColorToken",
        "Length",
        "Length",
        "Bool",
        null,
        "Text",
        "Text"
    ]);
    const WEB_KINDS = Object.freeze([
        null,
        Object.freeze({
            tag: "div",
            cls: "ewe-stack",
            attrs: Object.freeze([])
        }),
        Object.freeze({
            tag: "span",
            cls: "ewe-text",
            attrs: Object.freeze([])
        }),
        Object.freeze({
            tag: "img",
            cls: "ewe-image",
            attrs: Object.freeze([])
        }),
        Object.freeze({
            tag: "button",
            cls: "ewe-button",
            attrs: Object.freeze([
                Object.freeze([
                    "type",
                    "button"
                ])
            ])
        }),
        Object.freeze({
            tag: "input",
            cls: "ewe-text-input",
            attrs: Object.freeze([
                Object.freeze([
                    "type",
                    "text"
                ])
            ])
        }),
        Object.freeze({
            tag: "input",
            cls: "ewe-toggle",
            attrs: Object.freeze([
                Object.freeze([
                    "type",
                    "checkbox"
                ]),
                Object.freeze([
                    "role",
                    "switch"
                ])
            ])
        }),
        Object.freeze({
            tag: "div",
            cls: "ewe-list",
            attrs: Object.freeze([
                Object.freeze([
                    "role",
                    "list"
                ])
            ])
        }),
        Object.freeze({
            tag: "div",
            cls: "ewe-scroll",
            attrs: Object.freeze([])
        }),
        Object.freeze({
            tag: "div",
            cls: "ewe-spacer",
            attrs: Object.freeze([
                Object.freeze([
                    "aria-hidden",
                    "true"
                ])
            ])
        }),
        Object.freeze({
            tag: "div",
            cls: "ewe-overlay",
            attrs: Object.freeze([])
        }),
        Object.freeze({
            tag: "dialog",
            cls: "ewe-modal",
            attrs: Object.freeze([])
        }),
        Object.freeze({
            tag: "section",
            cls: "ewe-bottom-sheet",
            attrs: Object.freeze([])
        }),
        Object.freeze({
            tag: "h1",
            cls: "ewe-navigation-title",
            attrs: Object.freeze([])
        }),
        Object.freeze({
            tag: "button",
            cls: "ewe-toolbar-button",
            attrs: Object.freeze([
                Object.freeze([
                    "type",
                    "button"
                ])
            ])
        }),
        Object.freeze({
            tag: "div",
            cls: "ewe-segmented-control",
            attrs: Object.freeze([
                Object.freeze([
                    "role",
                    "group"
                ])
            ])
        }),
        Object.freeze({
            tag: "ul",
            cls: "ewe-inset-list",
            attrs: Object.freeze([])
        }),
        Object.freeze({
            tag: "li",
            cls: "ewe-checklist-row",
            attrs: Object.freeze([])
        }),
        Object.freeze({
            tag: "button",
            cls: "ewe-checkmark-button",
            attrs: Object.freeze([
                Object.freeze([
                    "type",
                    "button"
                ])
            ])
        }),
        Object.freeze({
            tag: "button",
            cls: "ewe-destructive-button",
            attrs: Object.freeze([
                Object.freeze([
                    "type",
                    "button"
                ])
            ])
        }),
        Object.freeze({
            tag: "p",
            cls: "ewe-list-footer",
            attrs: Object.freeze([])
        }),
        Object.freeze({
            tag: "dialog",
            cls: "ewe-confirmation-dialog",
            attrs: Object.freeze([
                Object.freeze([
                    "role",
                    "alertdialog"
                ])
            ])
        }),
        Object.freeze({
            tag: "h2",
            cls: "ewe-sheet-title",
            attrs: Object.freeze([])
        }),
        Object.freeze({
            tag: "button",
            cls: "ewe-submit-button",
            attrs: Object.freeze([
                Object.freeze([
                    "type",
                    "submit"
                ])
            ])
        }),
        Object.freeze({
            tag: "button",
            cls: "ewe-cancel-button",
            attrs: Object.freeze([
                Object.freeze([
                    "type",
                    "button"
                ])
            ])
        }),
        Object.freeze({
            tag: "canvas",
            cls: "ewe-drawing-surface",
            attrs: Object.freeze([])
        })
    ]);
    const WEB_EVENTS = Object.freeze([
        null,
        "click",
        "change",
        "submit",
        "focus",
        "blur"
    ]);
    const TAG_NAMES = Object.freeze([
        "div",
        "span",
        "input",
        "button",
        "html",
        "head",
        "body",
        "title",
        "base",
        "link",
        "meta",
        "style",
        "script",
        "noscript",
        "template",
        "slot",
        "main",
        "section",
        "nav",
        "article",
        "aside",
        "header",
        "footer",
        "address",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "hgroup",
        "p",
        "hr",
        "pre",
        "blockquote",
        "ol",
        "ul",
        "menu",
        "li",
        "dl",
        "dt",
        "dd",
        "figure",
        "figcaption",
        "search",
        "a",
        "em",
        "strong",
        "small",
        "s",
        "cite",
        "q",
        "dfn",
        "abbr",
        "ruby",
        "rt",
        "rp",
        "data",
        "time",
        "code",
        "var",
        "samp",
        "kbd",
        "sub",
        "sup",
        "i",
        "b",
        "u",
        "mark",
        "bdi",
        "bdo",
        "br",
        "wbr",
        "ins",
        "del",
        "picture",
        "source",
        "img",
        "iframe",
        "embed",
        "object",
        "video",
        "audio",
        "track",
        "map",
        "area",
        "svg",
        "math",
        "canvas",
        "table",
        "caption",
        "colgroup",
        "col",
        "tbody",
        "thead",
        "tfoot",
        "tr",
        "td",
        "th",
        "form",
        "label",
        "select",
        "datalist",
        "optgroup",
        "option",
        "textarea",
        "output",
        "progress",
        "meter",
        "fieldset",
        "legend",
        "details",
        "summary",
        "dialog"
    ]);
    const ATTR_NAMES = Object.freeze([
        "class",
        "id",
        "style",
        "value",
        "title",
        "lang",
        "dir",
        "hidden",
        "tabindex",
        "accesskey",
        "draggable",
        "contenteditable",
        "spellcheck",
        "translate",
        "role",
        "slot",
        "part",
        "is",
        "href",
        "src",
        "srcset",
        "sizes",
        "alt",
        "rel",
        "target",
        "download",
        "referrerpolicy",
        "crossorigin",
        "integrity",
        "loading",
        "media",
        "type",
        "name",
        "placeholder",
        "disabled",
        "readonly",
        "required",
        "checked",
        "selected",
        "multiple",
        "min",
        "max",
        "step",
        "pattern",
        "minlength",
        "maxlength",
        "autocomplete",
        "autofocus",
        "for",
        "form",
        "action",
        "method",
        "enctype",
        "novalidate",
        "accept",
        "rows",
        "cols",
        "wrap",
        "list",
        "size",
        "colspan",
        "rowspan",
        "headers",
        "scope",
        "width",
        "height",
        "controls",
        "autoplay",
        "loop",
        "muted",
        "preload",
        "poster",
        "playsinline",
        "charset",
        "content",
        "http-equiv",
        "open",
        "label",
        "datetime",
        "cite",
        "data"
    ]);
    const CORE_CSS = "[primal-id][hidden]{display:none!important}\n.ewe-stack{display:flex;flex-direction:column}\n.ewe-list{display:flex;flex-direction:column}\n.ewe-scroll{overflow:auto}\n.ewe-spacer{flex:1 1 auto}\n.ewe-drawing-surface{width:300px;height:150px}\n.ewe-navigation-title,.ewe-sheet-title{font-family:system-ui,sans-serif;line-height:1.2;letter-spacing:-.025em;margin:.3em 0}\n.ewe-toolbar-button,.ewe-submit-button,.ewe-cancel-button,.ewe-destructive-button,.ewe-checkmark-button,.ewe-segmented-control button{font:inherit;border:1px solid color-mix(in srgb,CanvasText 25%,Canvas);border-radius:.5em;padding:.45em .85em;cursor:pointer}\n.ewe-toolbar-button,.ewe-submit-button{align-self:flex-start;background:var(--color-primary,ButtonText);color:Canvas;color:contrast-color(var(--color-primary,ButtonText))}\n.ewe-cancel-button,.ewe-checkmark-button,.ewe-segmented-control button{background:Canvas;color:CanvasText}\n.ewe-destructive-button{background:Canvas;color:var(--color-error,#a51f26)}\n.ewe-segmented-control,.ewe-checklist-row{display:flex;align-items:center;flex-wrap:wrap;gap:.45em}\n.ewe-segmented-control{margin-block:.75em}\n.ewe-checklist-row{padding-block:.6em;border-block-end:1px solid color-mix(in srgb,CanvasText 15%,Canvas)}\n.ewe-checklist-row>.ewe-text{flex:1 1 auto}\n.ewe-inset-list{padding-inline-start:0;list-style:none;margin-block:.5em}\n.ewe-list-footer{font-size:.9em}\n.ewe-segmented-control button[aria-pressed=true]{background:var(--color-primary,ButtonText);color:Canvas;color:contrast-color(var(--color-primary,ButtonText));font-weight:600}\n.ewe-confirmation-dialog,.ewe-modal{font:inherit;color:CanvasText;background:Canvas;border:1px solid ButtonBorder;border-radius:.75em}\n.ewe-confirmation-dialog::backdrop,.ewe-modal::backdrop{background:rgb(0 0 0 / .35)}\n.ewe-confirmation-dialog>.ewe-sheet-title{margin-block-end:.8em}\n.ewe-overlay{position:fixed;inset:0;z-index:1;display:flex;flex-direction:column;pointer-events:none}\n.ewe-overlay[popover]{margin:0;border:0;padding:0;inline-size:auto;block-size:auto;overflow:visible;background:transparent;color:inherit}\n.ewe-overlay[popover]:not(:popover-open){display:none}\n.ewe-overlay>*{pointer-events:auto}\n.ewe-modal{max-block-size:100dvh;box-sizing:border-box}\n.ewe-modal:has(>.ewe-bottom-sheet){inset:auto 0 0;margin:0;inline-size:100%;max-inline-size:none;padding:0;border:0}\n.ewe-bottom-sheet{display:flex;flex-direction:column;margin-block-start:auto;max-block-size:100dvh;overflow:auto;padding-block-end:env(safe-area-inset-bottom);box-sizing:border-box;background:Canvas;color:CanvasText}\n.ewe-bottom-sheet:has(>.ewe-submit-button){display:grid;grid-template-columns:minmax(0,1fr) auto;gap:.8em;padding:1.25em max(1.25em,calc((100% - 36em)/2)) max(1.25em,env(safe-area-inset-bottom))}\n.ewe-bottom-sheet:has(>.ewe-submit-button)>*{grid-column:1/-1}\n.ewe-bottom-sheet:has(>.ewe-submit-button)>.ewe-cancel-button{grid-column:1;justify-self:end}\n.ewe-bottom-sheet:has(>.ewe-submit-button)>.ewe-submit-button{grid-column:2}\n.ewe-bottom-sheet>.ewe-text-input{font:inherit;padding:.65em .8em;border:1px solid GrayText;border-radius:.5em}\n";
    class ViewRecovery {
        constructor(load, install, { maxAttempts = 3, onError = ()=>{} } = {}){
            if (!Number.isSafeInteger(maxAttempts) || maxAttempts < 1) throw new RangeError("Invalid recovery attempt limit");
            this.load = load;
            this.install = install;
            this.maxAttempts = maxAttempts;
            this.onError = onError;
            this.attempts = 0;
            this.generation = 0;
            this.active = true;
            this.ready = false;
            this.expiredStreams = new Set();
        }
        open() {
            return this.run();
        }
        expire(stream) {
            if (!this.active) return;
            this.expiredStreams.add(stream);
            if (this.flight) return this.flight;
            if (this.attempts >= this.maxAttempts) {
                this.ready = false;
                this.onError(new Error("View recovery attempt limit reached"));
                return;
            }
            this.attempts++;
            return this.run();
        }
        retry() {
            if (this.flight) return this.flight;
            this.attempts = 0;
            return this.run();
        }
        run() {
            if (!this.active || this.flight) return this.flight;
            const generation = ++this.generation;
            this.ready = false;
            const controller = new AbortController();
            this.abort = controller;
            this.flight = Promise.resolve().then(()=>this.load(controller.signal)).then((bytes)=>{
                if (!this.active || this.generation !== generation) return;
                this.ready = true;
                this.install(bytes, this.expiredStreams);
                this.expiredStreams.clear();
            }).catch((error)=>{
                if (!this.active || this.generation !== generation) return;
                this.ready = false;
                this.onError(error);
                throw error;
            }).finally(()=>{
                if (this.generation === generation) this.flight = null;
            });
            return this.flight;
        }
        accepts(generation) {
            return this.active && this.ready && generation === this.generation;
        }
        close() {
            this.active = false;
            this.ready = false;
            this.generation++;
            this.abort?.abort();
            this.expiredStreams.clear();
        }
    }
    function composite(scope) {
        var root = scope.parent();
        if (!root || root.__composite) return;
        root.__composite = true;
        var orientation = root.getAttribute('data-orientation') || 'horizontal';
        var loop = root.getAttribute('data-loop') !== 'false';
        function items() {
            return Array.prototype.slice.call(root.querySelectorAll('[data-composite-item]')).filter(function(el) {
                return !el.hasAttribute('disabled') && el.getAttribute('aria-disabled') !== 'true';
            });
        }
        function seed() {
            var list = items();
            var active = list.findIndex(function(el) {
                return el.hasAttribute('data-composite-active');
            });
            if (active < 0) active = 0;
            list.forEach(function(el, i) {
                el.tabIndex = i === active ? 0 : -1;
            });
        }
        function move(idx) {
            var list = items();
            if (idx < 0) idx = loop ? list.length - 1 : 0;
            if (idx >= list.length) idx = loop ? 0 : list.length - 1;
            list.forEach(function(el, i) {
                el.tabIndex = i === idx ? 0 : -1;
            });
            if (list[idx]) {
                list[idx].focus();
                if (root.hasAttribute('data-composite-select')) list[idx].click();
            }
        }
        var nextKey = orientation === 'vertical' ? 'ArrowDown' : 'ArrowRight';
        var prevKey = orientation === 'vertical' ? 'ArrowUp' : 'ArrowLeft';
        var buf = '', bufAt = 0, matchIdx = -1;
        function label(el) {
            return (el.getAttribute('data-label') || el.textContent || '').trim().toLowerCase();
        }
        function typeahead(ch) {
            var list = items();
            if (!list.length) return;
            var now = Date.now();
            if (now - bufAt > 750) {
                buf = '';
            }
            bufAt = now;
            var cur = list.indexOf(document.activeElement);
            if (buf === '') matchIdx = cur;
            var allowRapid = list.every(function(el) {
                var t = label(el);
                return t.length < 2 || t[0] !== t[1];
            });
            if (allowRapid && buf === ch.toLowerCase()) {
                buf = '';
            }
            buf += ch.toLowerCase();
            var start = (matchIdx >= 0 ? matchIdx : cur) + 1;
            for(var k = 0; k < list.length; k++){
                var i = ((start + k) % list.length + list.length) % list.length;
                if (label(list[i]).indexOf(buf) === 0) {
                    matchIdx = i;
                    move(i);
                    return;
                }
            }
            buf = '';
        }
        scope.addEvent(root, 'keydown', function(e) {
            var list = items();
            var cur = list.indexOf(document.activeElement);
            var next = null;
            if (e.key === nextKey) next = cur + 1;
            else if (e.key === prevKey) next = cur - 1;
            else if (e.key === 'Home') next = 0;
            else if (e.key === 'End') next = list.length - 1;
            else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
                e.preventDefault();
                typeahead(e.key);
                return;
            } else return;
            if (cur < 0 && e.key !== 'Home' && e.key !== 'End') return;
            e.preventDefault();
            move(next);
        });
        seed();
    }
    function dialog(scope) {
        var dlg = scope.parent();
        if (!dlg || dlg.__dialog || !dlg.showModal) return;
        dlg.__dialog = true;
        var modal = dlg.getAttribute('data-dialog-mode') !== 'nonmodal';
        function sync() {
            var want = dlg.hasAttribute('data-open');
            if (want && !dlg.open) {
                try {
                    modal ? dlg.showModal() : dlg.show();
                } catch (e) {}
            } else if (!want && dlg.open) {
                dlg.close();
            }
        }
        function requestClose(reason) {
            var act = dlg.querySelector('[data-dialog-close]');
            if (act) {
                act.setAttribute('data-dismiss-reason', reason);
                act.click();
            }
        }
        scope.addEvent(dlg, 'cancel', function(e) {
            e.preventDefault();
            requestClose('escape-key');
        });
        scope.addEvent(dlg, 'click', function(e) {
            if (e.target === dlg && dlg.getAttribute('data-dialog-dismissable') !== 'false') {
                requestClose('outside-press');
            }
        });
        sync();
        var mo = new MutationObserver(sync);
        scope.onDispose(()=>mo.disconnect());
        mo.observe(dlg, {
            attributes: true,
            attributeFilter: [
                'data-open',
                'data-closed'
            ]
        });
    }
    function dismiss(scope) {
        var popup = scope.parent();
        if (!popup || popup.__dismiss) return;
        popup.__dismiss = true;
        var doc = popup.ownerDocument || document;
        function dismiss(reason) {
            var act = popup.querySelector('[data-dismiss-action]');
            if (!act) return;
            act.setAttribute('data-dismiss-reason', reason);
            act.click();
        }
        if (popup.getAttribute('data-dismiss-escape') !== 'false') {
            scope.addEvent(doc, 'keydown', function(e) {
                if (e.key !== 'Escape' && e.key !== 'Esc') return;
                e.preventDefault();
                dismiss('escape-key');
            });
        }
        if (popup.getAttribute('data-dismiss-outside') !== 'false') {
            scope.addEvent(doc, 'pointerdown', function(e) {
                if (popup.contains(e.target)) return;
                var anchor = popup.getAttribute('data-anchor');
                if (anchor) {
                    var trig = doc.getElementById(anchor);
                    if (trig && trig.contains(e.target)) return;
                }
                dismiss('outside-press');
            });
        }
    }
    function focus_trap(scope) {
        var el = scope.parent();
        if (!el || el.__focusTrap) return;
        el.__focusTrap = true;
        var doc = el.ownerDocument || document;
        var SEL = 'a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])';
        function tabbables() {
            return Array.prototype.slice.call(el.querySelectorAll(SEL)).filter(function(n) {
                return n.offsetWidth || n.offsetHeight || n.getClientRects().length;
            });
        }
        var restore = null;
        function activate() {
            restore = doc.activeElement;
            var init = el.getAttribute('data-initial-focus');
            if (init === 'none') return;
            var node = init ? el.querySelector(init) : tabbables()[0] || el;
            if (node && node.focus) node.focus();
        }
        function deactivate() {
            var node = restore && restore.focus ? restore : null;
            if (!node) {
                var anchor = el.getAttribute('data-anchor');
                if (anchor) {
                    var t = doc.getElementById(anchor);
                    if (t) node = t;
                }
            }
            if (node && node.focus) node.focus();
            restore = null;
        }
        scope.addEvent(el, 'keydown', function(e) {
            if (e.key !== 'Tab') return;
            var list = tabbables();
            if (!list.length) {
                e.preventDefault();
                return;
            }
            var first = list[0], last = list[list.length - 1];
            if (e.shiftKey && doc.activeElement === first) {
                e.preventDefault();
                last.focus();
            } else if (!e.shiftKey && doc.activeElement === last) {
                e.preventDefault();
                first.focus();
            }
        });
        var isOpen = el.hasAttribute('data-open');
        if (isOpen) activate();
        var mo = new MutationObserver(function() {
            var now = el.hasAttribute('data-open');
            if (now === isOpen) return;
            isOpen = now;
            if (now) activate();
            else deactivate();
        });
        scope.onDispose(()=>mo.disconnect());
        mo.observe(el, {
            attributes: true,
            attributeFilter: [
                'data-open',
                'data-closed'
            ]
        });
    }
    function slider_drag(scope) {
        var root = scope.parent();
        if (!root || root.__slider) return;
        root.__slider = true;
        var win = root.ownerDocument && root.ownerDocument.defaultView || window;
        var control = root.querySelector('[data-slider-control]');
        var input = root.querySelector('[data-slider-input]');
        if (!control || !input) return;
        function num(a) {
            var v = parseFloat(root.getAttribute(a));
            return isNaN(v) ? null : v;
        }
        var min = num('data-min');
        if (min == null) min = 0;
        var max = num('data-max');
        if (max == null) max = 100;
        var step = num('data-step') || 1;
        var largeStep = num('data-large-step') || 10;
        var vertical = root.getAttribute('data-orientation') === 'vertical';
        var rtl = root.getAttribute('dir') === 'rtl';
        function roundStep(v) {
            return Math.round((v - min) / step) * step + min;
        }
        function clampV(v) {
            return Math.max(min, Math.min(max, v));
        }
        function fingerValue(e) {
            var rect = control.getBoundingClientRect(), pct;
            if (vertical) pct = (rect.bottom - e.clientY) / rect.height;
            else pct = rtl ? (rect.right - e.clientX) / rect.width : (e.clientX - rect.left) / rect.width;
            pct = Math.max(0, Math.min(1, pct));
            return clampV(roundStep((max - min) * pct + min));
        }
        function commit(v) {
            input.value = String(v);
            input.dispatchEvent(new Event('change', {
                bubbles: true
            }));
        }
        var dragging = false;
        scope.addEvent(control, 'pointerdown', function(e) {
            if (e.button) return;
            e.preventDefault();
            dragging = true;
            root.setAttribute('data-dragging', '');
            if (control.setPointerCapture) {
                try {
                    control.setPointerCapture(e.pointerId);
                } catch (_) {}
            }
            if (input.focus) input.focus();
            commit(fingerValue(e));
        });
        scope.addEvent(win, 'pointermove', function(e) {
            if (dragging) commit(fingerValue(e));
        });
        scope.addEvent(win, 'pointerup', function() {
            if (!dragging) return;
            dragging = false;
            root.removeAttribute('data-dragging');
        });
        scope.addEvent(input, 'keydown', function(e) {
            var cur = parseFloat(input.value);
            if (isNaN(cur)) cur = min;
            if (e.key === 'PageUp') {
                e.preventDefault();
                commit(clampV(cur + largeStep));
            } else if (e.key === 'PageDown') {
                e.preventDefault();
                commit(clampV(cur - largeStep));
            }
        });
    }
    function swipe(scope) {
        var el = scope.parent();
        if (!el || el.__swipe) return;
        el.__swipe = true;
        el.ownerDocument && el.ownerDocument.defaultView || window;
        var dir = el.getAttribute('data-swipe-direction') || 'down';
        var prefix = el.getAttribute('data-swipe-prefix') || '--swipe';
        var threshold = parseFloat(el.getAttribute('data-swipe-threshold')) || 50;
        var horizontal = dir === 'left' || dir === 'right';
        var sign = dir === 'up' || dir === 'left' ? -1 : 1;
        var startX = 0, startY = 0, swiping = false, mx = 0, my = 0;
        function setVars() {
            el.style.setProperty(prefix + '-movement-x', mx + 'px');
            el.style.setProperty(prefix + '-movement-y', my + 'px');
            var along = horizontal ? mx : my;
            var progress = Math.max(0, Math.min(1, along * sign / Math.max(1, el.offsetHeight || el.offsetWidth)));
            el.style.setProperty('--drawer-swipe-progress', String(progress));
        }
        function reset() {
            mx = 0;
            my = 0;
            setVars();
        }
        reset();
        scope.addEvent(el, 'pointerdown', function(e) {
            if (e.button) return;
            swiping = true;
            startX = e.clientX;
            startY = e.clientY;
            el.setAttribute('data-swiping', '');
            el.setAttribute('data-swipe-direction', dir);
            if (el.setPointerCapture) {
                try {
                    el.setPointerCapture(e.pointerId);
                } catch (_) {}
            }
        });
        scope.addEvent(el, 'pointermove', function(e) {
            if (!swiping) return;
            var dx = e.clientX - startX, dy = e.clientY - startY;
            mx = horizontal ? sign > 0 ? Math.max(0, dx) : Math.min(0, dx) : 0;
            my = horizontal ? 0 : sign > 0 ? Math.max(0, dy) : Math.min(0, dy);
            setVars();
        });
        scope.addEvent(el, 'pointerup', function() {
            if (!swiping) return;
            swiping = false;
            el.removeAttribute('data-swiping');
            var along = (horizontal ? mx : my) * sign;
            if (along >= threshold) {
                var act = el.querySelector('[data-swipe-dismiss]');
                if (act) act.click();
            }
            reset();
        });
    }
    function snap(scope) {
        var el = scope.parent();
        if (!el || el.__snap) return;
        el.__snap = true;
        var dir = el.getAttribute('data-swipe-direction') || 'bottom';
        function sign() {
            return dir === 'up' || dir === 'left' ? -1 : 1;
        }
        var horizontal = dir === 'left' || dir === 'right';
        var points = (el.getAttribute('data-snap-points') || '1').split(',').map(parseFloat).filter(function(n) {
            return !isNaN(n);
        }).sort(function(a, b) {
            return a - b;
        });
        if (!points.length) points = [
            1
        ];
        var sequential = el.getAttribute('data-snap-sequential') === 'true';
        var snapInput = el.querySelector('[data-snap-input]');
        var full = horizontal ? el.offsetWidth : el.offsetHeight;
        var startCoord = 0, dragging = false, curIdx = points.length - 1, base = 0;
        function toOffset(f) {
            return (1 - f) * full;
        }
        function setOffset(px) {
            el.style.setProperty('--drawer-offset', px + 'px');
            el.style.setProperty('--drawer-snap-progress', String(full ? 1 - px / full : 1));
        }
        setOffset(toOffset(points[curIdx]));
        scope.addEvent(el, 'pointerdown', function(e) {
            if (e.button) return;
            dragging = true;
            startCoord = horizontal ? e.clientX : e.clientY;
            base = toOffset(points[curIdx]);
            el.setAttribute('data-swiping', '');
            if (el.setPointerCapture) {
                try {
                    el.setPointerCapture(e.pointerId);
                } catch (_) {}
            }
        });
        scope.addEvent(el, 'pointermove', function(e) {
            if (!dragging) return;
            var d = (horizontal ? e.clientX : e.clientY) - startCoord;
            var off = Math.max(0, Math.min(base + sign() * d, full));
            setOffset(off);
        });
        scope.addEvent(el, 'pointerup', function() {
            if (!dragging) return;
            dragging = false;
            el.removeAttribute('data-swiping');
            var off = parseFloat(el.style.getPropertyValue('--drawer-offset')) || 0;
            var frac = full ? 1 - off / full : 1;
            if (frac < points[0] / 2) {
                var act = el.querySelector('[data-swipe-dismiss]');
                if (act) {
                    act.click();
                    return;
                }
            }
            var best = 0, bestD = Infinity;
            for(var i = 0; i < points.length; i++){
                var dd = Math.abs(points[i] - frac);
                if (dd < bestD) {
                    bestD = dd;
                    best = i;
                }
            }
            if (sequential) best = Math.max(curIdx - 1, Math.min(curIdx + 1, best));
            curIdx = best;
            setOffset(toOffset(points[curIdx]));
            if (snapInput) {
                snapInput.value = String(curIdx);
                snapInput.dispatchEvent(new Event('change', {
                    bubbles: true
                }));
            }
        });
    }
    function hover(scope) {
        var trig = scope.parent();
        if (!trig || trig.__hover) return;
        trig.__hover = true;
        var doc = trig.ownerDocument || document;
        var win = doc.defaultView || window;
        var root = trig.parentNode;
        var openDelay = parseInt(trig.getAttribute('data-hover-delay'), 10) || 0;
        var closeDelay = parseInt(trig.getAttribute('data-hover-close-delay'), 10) || 0;
        function floating() {
            return root && root.querySelector('[data-hover-popup]');
        }
        function fire(sel) {
            var a = root && root.querySelector(sel);
            if (a) a.click();
        }
        var openT = null, closeT = null, polyT = null, moveHandler = null;
        function clearOpen() {
            if (openT) {
                win.clearTimeout(openT);
                openT = null;
            }
        }
        function clearClose() {
            if (closeT) {
                win.clearTimeout(closeT);
                closeT = null;
            }
        }
        function clearPoly() {
            if (polyT) {
                win.clearTimeout(polyT);
                polyT = null;
            }
        }
        function removeMove() {
            if (moveHandler) {
                doc.removeEventListener('mousemove', moveHandler);
                moveHandler = null;
            }
            clearPoly();
        }
        scope.onDispose(()=>{
            clearOpen();
            clearClose();
            removeMove();
        });
        function open() {
            clearClose();
            removeMove();
            clearOpen();
            openT = win.setTimeout(function() {
                fire('[data-hover-open]');
            }, openDelay);
        }
        function scheduleClose() {
            removeMove();
            clearClose();
            closeT = win.setTimeout(function() {
                fire('[data-hover-close]');
            }, closeDelay);
        }
        var BUF = 0.5, SPEED = 0.1;
        function edge(px, py, xi, yi, xj, yj) {
            return yi >= py !== yj >= py && px <= (xj - xi) * (py - yi) / (yj - yi) + xi;
        }
        function inQuad(px, py, x1, y1, x2, y2, x3, y3, x4, y4) {
            var s = false;
            if (edge(px, py, x1, y1, x2, y2)) s = !s;
            if (edge(px, py, x2, y2, x3, y3)) s = !s;
            if (edge(px, py, x3, y3, x4, y4)) s = !s;
            if (edge(px, py, x4, y4, x1, y1)) s = !s;
            return s;
        }
        function inRect(px, py, r) {
            return px >= r.left && px <= r.right && py >= r.top && py <= r.bottom;
        }
        function inAA(px, py, x1, y1, x2, y2) {
            return px >= Math.min(x1, x2) && px <= Math.max(x1, x2) && py >= Math.min(y1, y2) && py <= Math.max(y1, y2);
        }
        function startPolygon(x, y) {
            var fl = floating();
            if (!fl) {
                scheduleClose();
                return;
            }
            var side = fl.getAttribute('data-side') || 'bottom';
            var landed = false, lastX = null, lastY = null, lastT = win.performance ? win.performance.now() : Date.now();
            function slow(nx, ny) {
                var t = win.performance ? win.performance.now() : Date.now(), dt = t - lastT;
                if (lastX === null || dt === 0) {
                    lastX = nx;
                    lastY = ny;
                    lastT = t;
                    return false;
                }
                var dx = nx - lastX, dy = ny - lastY, d2 = dx * dx + dy * dy, th = dt * dt * SPEED * SPEED;
                lastX = nx;
                lastY = ny;
                lastT = t;
                return d2 < th;
            }
            clearClose();
            moveHandler = function(e) {
                clearPoly();
                var refRect = trig.getBoundingClientRect(), rect = fl.getBoundingClientRect();
                var cx = e.clientX, cy = e.clientY, target = e.target;
                if (fl.contains(target)) {
                    landed = true;
                    return;
                }
                if (trig.contains(target)) {
                    landed = true;
                    return;
                }
                var leaveRight = x > rect.right - rect.width / 2, leaveBottom = y > rect.bottom - rect.height / 2;
                var wider = rect.width > refRect.width, taller = rect.height > refRect.height;
                var left = (wider ? refRect : rect).left, right = (wider ? refRect : rect).right;
                var top = (taller ? refRect : rect).top, bottom = (taller ? refRect : rect).bottom;
                if (side === 'top' && y >= refRect.bottom - 1 || side === 'bottom' && y <= refRect.top + 1 || side === 'left' && x >= refRect.right - 1 || side === 'right' && x <= refRect.left + 1) {
                    scheduleClose();
                    return;
                }
                var trough = false;
                if (side === 'top') trough = inAA(cx, cy, left, refRect.top + 1, right, rect.bottom - 1);
                else if (side === 'bottom') trough = inAA(cx, cy, left, rect.top + 1, right, refRect.bottom - 1);
                else if (side === 'left') trough = inAA(cx, cy, rect.right - 1, bottom, refRect.left + 1, top);
                else if (side === 'right') trough = inAA(cx, cy, refRect.right - 1, bottom, rect.left + 1, top);
                if (trough) return;
                if (landed && !inRect(cx, cy, refRect)) {
                    scheduleClose();
                    return;
                }
                if (slow(cx, cy)) {
                    scheduleClose();
                    return;
                }
                var inside = false;
                if (side === 'top' || side === 'bottom') {
                    var off = wider ? BUF / 2 : BUF * 4;
                    var p1x = wider ? x + off : leaveRight ? x + off : x - off;
                    var p2x = wider ? x - off : leaveRight ? x + off : x - off;
                    var py = side === 'top' ? y + BUF + 1 : y - BUF;
                    var nearY = side === 'top' ? rect.bottom - BUF : rect.top + BUF;
                    var farY = side === 'top' ? rect.top : rect.bottom;
                    var cyL = leaveRight ? nearY : wider ? nearY : farY;
                    var cyR = leaveRight ? wider ? nearY : farY : nearY;
                    inside = inQuad(cx, cy, p1x, py, p2x, py, rect.left, cyL, rect.right, cyR);
                } else {
                    var offY = taller ? BUF / 2 : BUF * 4;
                    var p1y = taller ? y + offY : leaveBottom ? y + offY : y - offY;
                    var p2y = taller ? y - offY : leaveBottom ? y + offY : y - offY;
                    var px = side === 'left' ? x + BUF + 1 : x - BUF;
                    var nearX = side === 'left' ? rect.right - BUF : rect.left + BUF;
                    var farX = side === 'left' ? rect.left : rect.right;
                    var cxT = leaveBottom ? nearX : taller ? nearX : farX;
                    var cxB = leaveBottom ? taller ? nearX : farX : nearX;
                    if (side === 'left') inside = inQuad(cx, cy, cxT, rect.top, cxB, rect.bottom, px, p1y, px, p2y);
                    else inside = inQuad(cx, cy, px, p1y, px, p2y, cxT, rect.top, cxB, rect.bottom);
                }
                if (!inside) {
                    scheduleClose();
                } else if (!landed) {
                    polyT = win.setTimeout(function() {
                        fire('[data-hover-close]');
                    }, 40);
                }
            };
            doc.addEventListener('mousemove', moveHandler);
        }
        scope.addEvent(trig, 'pointerenter', open);
        scope.addEvent(trig, 'pointerleave', function(e) {
            clearOpen();
            startPolygon(e.clientX, e.clientY);
        });
        scope.addEvent(trig, 'focus', function() {
            clearClose();
            removeMove();
            clearOpen();
            fire('[data-hover-open]');
        });
        scope.addEvent(trig, 'blur', function() {
            scheduleClose();
        });
        var fl0 = floating();
        if (fl0) {
            scope.addEvent(fl0, 'pointerenter', function() {
                clearClose();
                removeMove();
            });
            scope.addEvent(fl0, 'pointerleave', function(e) {
                startPolygon(e.clientX, e.clientY);
            });
        }
    }
    function listbox(scope) {
        var pop = scope.parent();
        if (!pop || pop.__listbox) return;
        pop.__listbox = true;
        var loop = pop.getAttribute('data-loop') !== 'false';
        var buf = '', bufAt = 0;
        function items() {
            return Array.prototype.slice.call(pop.querySelectorAll('[data-list-item]')).filter(function(el) {
                return !el.hasAttribute('disabled') && el.getAttribute('aria-disabled') !== 'true' && el.offsetParent !== null;
            });
        }
        function current() {
            var l = items();
            for(var i = 0; i < l.length; i++)if (l[i].hasAttribute('data-highlighted')) return i;
            return -1;
        }
        function setHL(idx) {
            var l = items();
            l.forEach(function(el) {
                el.removeAttribute('data-highlighted');
            });
            if (idx >= 0 && idx < l.length) {
                var el = l[idx];
                el.setAttribute('data-highlighted', '');
                if (el.id) pop.setAttribute('aria-activedescendant', el.id);
                if (el.scrollIntoView) el.scrollIntoView({
                    block: 'nearest'
                });
            } else {
                pop.removeAttribute('aria-activedescendant');
            }
        }
        function move(delta) {
            var l = items();
            if (!l.length) return;
            var idx = current() + delta;
            if (idx < 0) idx = loop ? l.length - 1 : 0;
            if (idx >= l.length) idx = loop ? 0 : l.length - 1;
            setHL(idx);
        }
        function typeahead(ch) {
            var now = Date.now();
            if (now - bufAt > 750) buf = '';
            bufAt = now;
            buf += ch.toLowerCase();
            var l = items();
            if (!l.length) return;
            var start = (current() + 1) % l.length;
            for(var k = 0; k < l.length; k++){
                var i = (start + k) % l.length;
                var label = (l[i].getAttribute('data-label') || l[i].textContent || '').trim().toLowerCase();
                if (label.indexOf(buf) === 0) {
                    setHL(i);
                    return;
                }
            }
        }
        scope.addEvent(pop, 'keydown', function(e) {
            if (e.key === 'ArrowDown') {
                e.preventDefault();
                move(1);
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                move(-1);
            } else if (e.key === 'Home') {
                e.preventDefault();
                setHL(0);
            } else if (e.key === 'End') {
                e.preventDefault();
                setHL(items().length - 1);
            } else if (e.key === 'Enter' || e.key === ' ') {
                var c = current();
                var l = items();
                if (c >= 0) {
                    e.preventDefault();
                    l[c].click();
                }
            } else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
                typeahead(e.key);
            }
        });
        scope.addEvent(pop, 'pointermove', function(e) {
            var item = e.target.closest ? e.target.closest('[data-list-item]') : null;
            if (item) {
                var i = items().indexOf(item);
                if (i >= 0) setHL(i);
            }
        });
        var l = items();
        var active = -1;
        for(var i = 0; i < l.length; i++)if (l[i].hasAttribute('data-active-item')) {
            active = i;
            break;
        }
        setHL(active);
    }
    function panel_size(scope) {
        var el = scope.parent();
        if (!el || el.__panelSize) return;
        el.__panelSize = true;
        var win = el.ownerDocument && el.ownerDocument.defaultView || window;
        var prefix = el.getAttribute('data-size-var') || '--panel';
        function measure() {
            el.style.setProperty(prefix + '-height', el.scrollHeight + 'px');
            el.style.setProperty(prefix + '-width', el.scrollWidth + 'px');
        }
        measure();
        if (win.ResizeObserver) {
            var ro = new win.ResizeObserver(measure);
            ro.observe(el);
            scope.onDispose(()=>ro.disconnect());
        }
        scope.addEvent(win, 'resize', measure);
    }
    function tab_indicator(scope) {
        var list = scope.parent();
        if (!list || list.__tabIndicator) return;
        list.__tabIndicator = true;
        var win = list.ownerDocument && list.ownerDocument.defaultView || window;
        function measure() {
            var tab = list.querySelector('[role="tab"][data-active]');
            if (!tab) return;
            var l = list.getBoundingClientRect(), t = tab.getBoundingClientRect();
            var s = list.style;
            s.setProperty('--active-tab-left', t.left - l.left + 'px');
            s.setProperty('--active-tab-right', l.right - t.right + 'px');
            s.setProperty('--active-tab-top', t.top - l.top + 'px');
            s.setProperty('--active-tab-bottom', l.bottom - t.bottom + 'px');
            s.setProperty('--active-tab-width', t.width + 'px');
            s.setProperty('--active-tab-height', t.height + 'px');
        }
        measure();
        var mo = new MutationObserver(measure);
        scope.onDispose(()=>mo.disconnect());
        mo.observe(list, {
            attributes: true,
            subtree: true,
            attributeFilter: [
                'data-active'
            ]
        });
        if (win.ResizeObserver) {
            var ro = new win.ResizeObserver(measure);
            ro.observe(list);
            scope.onDispose(()=>ro.disconnect());
        }
        scope.addEvent(win, 'resize', measure);
    }
    function position(scope) {
        var pos = scope.parent();
        if (!pos) return;
        var doc = pos.ownerDocument || document;
        var win = doc.defaultView || window;
        var anchorId = pos.getAttribute('data-anchor');
        var prefSide = pos.getAttribute('data-prefer-side') || 'bottom';
        var prefAlign = pos.getAttribute('data-prefer-align') || 'center';
        var offset = parseFloat(pos.getAttribute('data-offset')) || 0;
        var pad = parseFloat(pos.getAttribute('data-collision-padding'));
        if (isNaN(pad)) pad = 5;
        var sideMode = pos.getAttribute('data-collision-side') || 'flip';
        var alignMode = pos.getAttribute('data-collision-align') || 'flip';
        var fallbackAxis = pos.getAttribute('data-fallback-axis') || 'none';
        var sticky = pos.getAttribute('data-sticky') === 'true';
        var arrowPad = parseFloat(pos.getAttribute('data-arrow-padding'));
        if (isNaN(arrowPad)) arrowPad = 5;
        var rtl = pos.getAttribute('dir') === 'rtl';
        var track = pos.getAttribute('data-disable-anchor-tracking') !== 'true';
        function opposite(s) {
            return ({
                top: 'bottom',
                bottom: 'top',
                left: 'right',
                right: 'left'
            })[s];
        }
        function vertical(s) {
            return s === 'top' || s === 'bottom';
        }
        function anchorEl() {
            return anchorId ? doc.getElementById(anchorId) : pos.previousElementSibling;
        }
        function place() {
            var anchor = anchorEl();
            if (!anchor) return;
            var a = anchor.getBoundingClientRect();
            var p = pos.getBoundingClientRect();
            var w = p.width, h = p.height;
            var vw = doc.documentElement.clientWidth;
            var vh = doc.documentElement.clientHeight;
            var space = {
                top: a.top - pad,
                bottom: vh - a.bottom - pad,
                left: a.left - pad,
                right: vw - a.right - pad
            };
            var side = prefSide, align = prefAlign;
            var need = (vertical(side) ? h : w) + offset;
            if (sideMode === 'flip' && space[side] < need && space[opposite(side)] > space[side]) side = opposite(side);
            if (fallbackAxis !== 'none' && space[side] < need && space[opposite(side)] < need) {
                var perpNeed = (vertical(side) ? w : h) + offset;
                var perp = vertical(side) ? [
                    'left',
                    'right'
                ] : [
                    'top',
                    'bottom'
                ];
                if (vertical(side) && rtl) perp = [
                    'right',
                    'left'
                ];
                if (fallbackAxis === 'end') perp = [
                    perp[1],
                    perp[0]
                ];
                var chosen = null;
                for(var pi = 0; pi < perp.length; pi++){
                    if (space[perp[pi]] >= perpNeed) {
                        chosen = perp[pi];
                        break;
                    }
                }
                if (!chosen) chosen = space[perp[0]] >= space[perp[1]] ? perp[0] : perp[1];
                side = chosen;
            }
            var left, top;
            if (side === 'top') top = a.top - h - offset;
            else if (side === 'bottom') top = a.bottom + offset;
            else if (side === 'left') left = a.left - w - offset;
            else left = a.right + offset;
            function cross(al) {
                if (vertical(side)) return al === 'start' ? a.left : al === 'end' ? a.right - w : a.left + (a.width - w) / 2;
                return al === 'start' ? a.top : al === 'end' ? a.bottom - h : a.top + (a.height - h) / 2;
            }
            var lo = pad;
            var hi = vertical(side) ? vw - w - pad : vh - h - pad;
            var c = cross(align);
            if (alignMode === 'flip' && (align === 'start' || align === 'end')) {
                if (c < lo || c > hi) {
                    var alt = align === 'start' ? 'end' : 'start', c2 = cross(alt);
                    if (c2 >= lo && c2 <= hi) {
                        align = alt;
                        c = c2;
                    }
                }
            } else if (alignMode === 'shift') {
                if (c > hi) c = hi;
                if (c < lo) c = lo;
            }
            if (vertical(side)) left = c;
            else top = c;
            if (sticky) {
                left = Math.max(pad, Math.min(left, vw - w - pad));
                top = Math.max(pad, Math.min(top, vh - h - pad));
            }
            var fixed = pos.getAttribute('data-position-method') === 'fixed';
            pos.style.position = fixed ? 'fixed' : 'absolute';
            var sx = fixed ? 0 : win.scrollX || 0, sy = fixed ? 0 : win.scrollY || 0;
            pos.style.left = left + sx + 'px';
            pos.style.top = top + sy + 'px';
            pos.setAttribute('data-side', side);
            pos.setAttribute('data-align', align);
            var hidden = a.bottom < 0 || a.right < 0 || a.top > vh || a.left > vw;
            if (hidden && !sticky) pos.setAttribute('data-anchor-hidden', '');
            else pos.removeAttribute('data-anchor-hidden');
            var arrow = pos.querySelector('[data-arrow]');
            if (arrow) {
                var as = arrow.style;
                if (vertical(side)) {
                    var center = a.left + a.width / 2 - left;
                    var clamped = Math.max(arrowPad, Math.min(center, w - arrowPad));
                    as.setProperty('--arrow-x', clamped + 'px');
                    as.removeProperty('--arrow-y');
                    if (clamped !== center) arrow.setAttribute('data-uncentered', '');
                    else arrow.removeAttribute('data-uncentered');
                } else {
                    var centerY = a.top + a.height / 2 - top;
                    var clampedY = Math.max(arrowPad, Math.min(centerY, h - arrowPad));
                    as.setProperty('--arrow-y', clampedY + 'px');
                    as.removeProperty('--arrow-x');
                    if (clampedY !== centerY) arrow.setAttribute('data-uncentered', '');
                    else arrow.removeAttribute('data-uncentered');
                }
                arrow.setAttribute('data-side', side);
            }
            var st = pos.style;
            st.setProperty('--anchor-width', a.width + 'px');
            st.setProperty('--anchor-height', a.height + 'px');
            st.setProperty('--available-width', Math.max(0, vertical(side) ? vw - 2 * pad : space[side]) + 'px');
            st.setProperty('--available-height', Math.max(0, vertical(side) ? space[side] : vh - 2 * pad) + 'px');
            st.setProperty('--positioner-width', w + 'px');
            st.setProperty('--positioner-height', h + 'px');
            st.setProperty('--popup-width', w + 'px');
            st.setProperty('--popup-height', h + 'px');
            var ox = align === 'start' ? '0%' : align === 'end' ? '100%' : '50%';
            st.setProperty('--transform-origin', vertical(side) ? ox + ' ' + (side === 'top' ? '100%' : '0%') : (side === 'left' ? '100%' : '0%') + ' ' + ox);
        }
        place();
        if (track) {
            scope.addEvent(doc, 'scroll', place, true);
            scope.addEvent(win, 'resize', place);
        }
    }
    function scroll_lock(scope) {
        var el = scope.parent();
        if (!el || el.__scrollLock) return;
        el.__scrollLock = true;
        var doc = el.ownerDocument || document;
        var win = doc.defaultView || window;
        var html = doc.documentElement, body = doc.body;
        function targetEl() {
            var bs = win.getComputedStyle(body), oy = bs.overflowY;
            return oy === 'scroll' || oy === 'hidden' || oy === 'auto' ? body : html;
        }
        var saved = null;
        function lock() {
            if (saved) return;
            var t = targetEl();
            var stable = win.CSS && win.CSS.supports && win.CSS.supports('scrollbar-gutter', 'stable');
            saved = {
                t: t,
                overflowY: t.style.overflowY,
                gutter: t.style.scrollbarGutter,
                padding: t.style.paddingRight
            };
            t.style.overflowY = 'hidden';
            if (stable) {
                t.style.scrollbarGutter = 'stable';
            } else {
                var sbw = win.innerWidth - html.clientWidth;
                if (sbw > 0) t.style.paddingRight = parseFloat(win.getComputedStyle(t).paddingRight) + sbw + 'px';
            }
        }
        function unlock() {
            if (!saved) return;
            saved.t.style.overflowY = saved.overflowY;
            saved.t.style.scrollbarGutter = saved.gutter;
            saved.t.style.paddingRight = saved.padding;
            saved = null;
        }
        scope.onDispose(unlock);
        function sync() {
            if (el.hasAttribute('data-open')) lock();
            else unlock();
        }
        sync();
        var mo = new MutationObserver(sync);
        scope.onDispose(()=>mo.disconnect());
        mo.observe(el, {
            attributes: true,
            attributeFilter: [
                'data-open',
                'data-closed'
            ]
        });
    }
    function transition(scope) {
        var el = scope.parent();
        if (!el || el.__transition) return;
        el.__transition = true;
        var win = el.ownerDocument && el.ownerDocument.defaultView || window;
        function twoFrames(fn) {
            win.requestAnimationFrame(function() {
                win.requestAnimationFrame(fn);
            });
        }
        function onEnd(e) {
            if (e.target !== el) return;
            el.removeEventListener('transitionend', onEnd);
            el.removeAttribute('data-ending-style');
            var act = el.querySelector('[data-transition-complete]');
            if (act) act.click();
        }
        function enter() {
            el.removeAttribute('data-ending-style');
            el.removeEventListener('transitionend', onEnd);
            if (el.hasAttribute('data-instant')) return;
            el.setAttribute('data-starting-style', '');
            twoFrames(function() {
                el.removeAttribute('data-starting-style');
            });
        }
        function leave() {
            el.removeAttribute('data-starting-style');
            if (el.hasAttribute('data-instant')) {
                el.removeAttribute('data-ending-style');
                return;
            }
            el.setAttribute('data-ending-style', '');
            scope.addEvent(el, 'transitionend', onEnd);
        }
        var isOpen = el.hasAttribute('data-open');
        if (isOpen) enter();
        var mo = new MutationObserver(function() {
            var now = el.hasAttribute('data-open');
            if (now === isOpen) return;
            isOpen = now;
            if (now) enter();
            else leave();
        });
        scope.onDispose(()=>mo.disconnect());
        mo.observe(el, {
            attributes: true,
            attributeFilter: [
                'data-open',
                'data-closed'
            ]
        });
    }
    const active = new WeakMap();
    const custom = new Map();
    function machinery(fn) {
        return (node, args)=>{
            const removals = [];
            const scope = {
                parent: ()=>node,
                onDispose: (cleanup)=>removals.push(cleanup),
                args,
                addEvent: (target, name, callback, options)=>{
                    if (typeof target === "string") target = node.querySelector(target);
                    if (!target) return;
                    target.addEventListener(name, callback, options);
                    removals.push(()=>target.removeEventListener(name, callback, options));
                }
            };
            fn(scope);
            return ()=>{
                for (const remove of removals)remove();
            };
        };
    }
    const SHARED = Object.freeze({
        DIALOG: machinery(dialog),
        DISMISS: machinery(dismiss),
        SCROLL_LOCK: machinery(scroll_lock),
        TRANSITION: machinery(transition)
    });
    const WEB_ONLY = Object.freeze({
        composite: machinery(composite),
        focus_trap: machinery(focus_trap),
        gestures: machinery((scope)=>{
            const mode = scope.args.find((arg)=>arg.name === "mode")?.value?.v;
            (({
                slider: slider_drag,
                swipe,
                snap
            })[mode] ?? slider_drag)(scope);
        }),
        hover: machinery(hover),
        listbox: machinery(listbox),
        measure: machinery((scope)=>{
            const mode = scope.args.find((arg)=>arg.name === "mode")?.value?.v;
            (mode === "tab_indicator" ? tab_indicator : panel_size)(scope);
        }),
        position: machinery(position)
    });
    function registerWebBehaviour(name, fn) {
        if (typeof fn !== "function") throw new TypeError("Expected a behaviour function");
        custom.set(name, fn);
    }
    function apply(node, name, args, run) {
        if (!run) return false;
        let entries = active.get(node);
        if (!entries) active.set(node, entries = new Map());
        if (!entries.has(name)) entries.set(name, run(node, args));
        return true;
    }
    function applyBehaviour(node, id, args = []) {
        const name = BehaviourId[id];
        return apply(node, name, args, SHARED[name]);
    }
    function applyWebBehaviour(node, name, args = []) {
        const mode = name === "gestures" || name === "measure" ? args?.find((arg)=>arg.name === "mode")?.value?.v : undefined;
        const key = mode === undefined ? name : name + ":" + mode;
        return apply(node, key, args, Object.hasOwn(WEB_ONLY, name) ? WEB_ONLY[name] : custom.get(name));
    }
    function disposeBehaviours(node) {
        const entries = active.get(node);
        active.delete(node);
        if (entries) {
            for (const cleanup of entries.values())if (typeof cleanup === "function") cleanup();
        }
    }
    function hydrateMarkers(root) {
        for (const marker of root.querySelectorAll?.("span[data-ewe-behaviour]") ?? []){
            const parent = marker.parentElement ?? marker.parentNode;
            if (!parent) continue;
            const name = marker.getAttribute("data-ewe-behaviour");
            const mode = marker.getAttribute("data-ewe-mode");
            const args = mode === null ? [] : [
                {
                    name: "mode",
                    value: {
                        v: mode
                    }
                }
            ];
            const shared = BehaviourId[name.toUpperCase()];
            const applied = shared ? applyBehaviour(parent, shared, args) : applyWebBehaviour(parent, name, args);
            if (!applied) console.error("Unknown component behaviour", name);
            marker.remove();
        }
    }
    const mod2 = {
        SHARED: SHARED,
        WEB_ONLY: WEB_ONLY,
        registerWebBehaviour: registerWebBehaviour,
        applyBehaviour: applyBehaviour,
        applyWebBehaviour: applyWebBehaviour,
        disposeBehaviours: disposeBehaviours,
        hydrateMarkers: hydrateMarkers
    };
    const asBytes = (input)=>ArrayBuffer.isView(input) ? new Uint8Array(input.buffer, input.byteOffset, input.byteLength) : new Uint8Array(input);
    function requestBytes(session, request) {
        const builder = new Builder();
        const frame = new FrameT(1, session, 0n, FrameKind.FrameRequest, request);
        Frame.finishFrameBuffer(builder, frame.pack(builder));
        return builder.asUint8Array();
    }
    const utf8 = new TextDecoder("utf-8", {
        fatal: true
    });
    const defaultFrameBytes = 4 * 1024 * 1024;
    function isFrame(input) {
        const b = asBytes(input);
        return b.length >= 8 && b[4] === 69 && b[5] === 87 && b[6] === 70 && b[7] === 49;
    }
    class FrameSplitter {
        constructor(limit){
            this.limit = limit;
            this.prefix = new Uint8Array(4);
            this.used = 0;
            this.frame = null;
        }
        *push(input) {
            const bytes = asBytes(input);
            let cursor = 0;
            while(cursor < bytes.length){
                const dest = this.frame ?? this.prefix;
                const count = Math.min(dest.length - this.used, bytes.length - cursor);
                dest.set(bytes.subarray(cursor, cursor + count), this.used);
                this.used += count;
                cursor += count;
                if (this.used !== dest.length) continue;
                this.used = 0;
                if (this.frame) {
                    this.frame = null;
                    if (!isFrame(dest)) throw new Error("Invalid EWF1 identifier");
                    yield dest;
                } else {
                    const size = new DataView(dest.buffer).getUint32(0, true);
                    if (size < 8 || size > this.limit) throw new RangeError("Invalid frame size");
                    this.frame = new Uint8Array(size);
                }
            }
        }
        finish() {
            if (this.frame || this.used) throw new Error("Truncated frame");
        }
    }
    function splitFrames(input, limit = defaultFrameBytes) {
        const splitter = new FrameSplitter(limit);
        const frames = Array.from(splitter.push(input));
        splitter.finish();
        return frames;
    }
    async function* readFrames(body, limit = defaultFrameBytes) {
        const splitter = new FrameSplitter(limit);
        if (!body) throw new Error("Missing frame response body");
        const reader = body.getReader();
        let done = false;
        try {
            while(true){
                const chunk = await reader.read();
                if (chunk.done) {
                    splitter.finish();
                    done = true;
                    break;
                }
                yield* splitter.push(chunk.value);
            }
        } finally{
            try {
                if (!done) await reader.cancel();
            } finally{
                reader.releaseLock();
            }
        }
    }
    async function readViewResponse(response, limit = defaultFrameBytes) {
        if (!response.ok) throw new Error("View request failed with HTTP " + response.status);
        if (!response.body) throw new Error("Missing view response body");
        const reader = response.body.getReader();
        const chunks = [];
        let size = 0, complete = false;
        try {
            while(true){
                const { done, value } = await reader.read();
                if (done) {
                    complete = true;
                    break;
                }
                size += value.byteLength;
                if (size > limit) throw new RangeError("View exceeds frame limit");
                chunks.push(value);
            }
        } finally{
            try {
                if (!complete) await reader.cancel();
            } finally{
                reader.releaseLock();
            }
        }
        const frame = new Uint8Array(size);
        let offset = 0;
        for (const chunk of chunks){
            frame.set(chunk, offset);
            offset += chunk.byteLength;
        }
        if (!isFrame(frame)) throw new Error("Invalid view frame");
        return frame;
    }
    function base64urlEncode(bytes) {
        let binary = "";
        for (const value of bytes)binary += String.fromCharCode(value);
        return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
    }
    class CheckedBuffer extends mod1.ByteBuffer {
        check(pos, length) {
            if (pos < 0 || length < 0 || pos + length > this.bytes().length) throw new Error("Truncated frame field");
        }
        readInt8(pos) {
            this.check(pos, 1);
            return super.readInt8(pos);
        }
        readUint8(pos) {
            this.check(pos, 1);
            return super.readUint8(pos);
        }
        readInt16(pos) {
            this.check(pos, 2);
            return super.readInt16(pos);
        }
        readUint16(pos) {
            this.check(pos, 2);
            return super.readUint16(pos);
        }
        readInt32(pos) {
            this.check(pos, 4);
            return super.readInt32(pos);
        }
        readUint32(pos) {
            this.check(pos, 4);
            return super.readUint32(pos);
        }
        __vector_len(pos) {
            const len = super.__vector_len(pos);
            this.check(super.__vector(pos), len);
            return len;
        }
        __string(pos, encoding) {
            const start = pos + this.readInt32(pos);
            this.check(start + 4, this.readInt32(start));
            return super.__string(pos, encoding);
        }
    }
    function vector(table, field, vtableOffset, Type, stats) {
        const count = table[field + "Length"]();
        const bb = table.bb;
        const offset = bb.__offset(table.bb_pos, vtableOffset);
        if (!offset) return new Type();
        const start = bb.__vector(table.bb_pos + offset);
        bb.check(start, count * Type.BYTES_PER_ELEMENT);
        if ((bb.bytes().byteOffset + start) % Type.BYTES_PER_ELEMENT === 0) {
            if (stats) stats.zeroCopyColumns++;
            return table[field + "Array"]?.() ?? new Type(bb.bytes().buffer, bb.bytes().byteOffset + start, count);
        }
        if (stats) stats.copiedColumns++;
        return Type.from({
            length: count
        }, (_, i)=>table[field](i));
    }
    function problem(code, fields = {}) {
        return Object.assign(new Error(code), {
            report: {
                code,
                ...fields
            }
        });
    }
    const utilityClasses = new WeakMap();
    function applyClasses(node, classes) {
        const previous = utilityClasses.get(node) ?? [];
        for (const cls of previous)node.classList.remove(cls);
        const words = (classes ?? "").split(/\s+/).filter(Boolean);
        const shared = words.filter((word)=>!/^(ios|android|web)-/.test(word));
        const overrides = words.filter((word)=>word.startsWith("web-")).map((word)=>word.slice(4));
        const ordered = [
            ...shared,
            ...overrides
        ];
        const applied = [];
        for (const word of ordered){
            if (!node.classList.contains(word)) {
                node.classList.add(word);
                applied.push(word);
            }
        }
        node.setAttribute("data-ewe-themed", "");
        for (const [prefix, categories, field] of [
            [
                "text-",
                [
                    "color"
                ],
                "color"
            ],
            [
                "text-",
                [
                    "font-size"
                ],
                "font-size"
            ],
            [
                "bg-",
                [
                    "color"
                ],
                "background"
            ],
            [
                "p-",
                [
                    "padding",
                    "spacing"
                ],
                "padding"
            ]
        ]){
            let value = "";
            for (const word of ordered){
                const name = word.slice(prefix.length);
                if (!word.startsWith(prefix) || !/^[a-zA-Z0-9_-]+$/.test(name)) continue;
                for (const category of categories)value = "var(--" + category + "-" + name + (value ? "," + value : "") + ")";
            }
            const property = "--ewe-utility-" + field;
            if (value) node.style.setProperty?.(property, value);
            else node.style.removeProperty?.(property);
        }
        utilityClasses.set(node, applied);
    }
    const propStyles = {
        PADDING: "padding",
        SPACING: "gap",
        WIDTH: "width",
        HEIGHT: "height"
    };
    function applyProp(node, id, type, value) {
        if (!value || mod1.PropValue[type] !== PROP_KIND[id]) throw problem("BadFrame");
        const prop = mod1.PropId[id];
        const v = value.v;
        if (Object.hasOwn(propStyles, prop)) {
            node.style[propStyles[prop]] = v.unit === mod1.LengthUnit.Auto ? "auto" : String(v.value) + (v.unit === mod1.LengthUnit.Percent ? "%" : "px");
            return;
        }
        switch(prop){
            case "ID":
                node.setAttribute("id", v);
                break;
            case "ALIGN":
                node.style.alignItems = [
                    "flex-start",
                    "center",
                    "flex-end",
                    "stretch"
                ][v];
                break;
            case "AXIS":
                node.style.flexDirection = v === mod1.Axis.Horizontal ? "row" : "column";
                break;
            case "FONT":
                node.style.font = "var(--ewe-font-" + value.id + ")";
                break;
            case "COLOR":
            case "BACKGROUND":
                node.style[prop === "COLOR" ? "color" : "backgroundColor"] = "var(--ewe-color-" + value.id + ")";
                break;
            case "ENABLED":
            case "HIDDEN":
                {
                    const attr = prop === "HIDDEN" ? "hidden" : "disabled";
                    if (prop === "HIDDEN" ? v : !v) node.setAttribute(attr, "");
                    else node.removeAttribute(attr);
                    break;
                }
            case "UTILITY_CLASSES":
                applyClasses(node, v);
                break;
            case "A11Y_LABEL":
                node.setAttribute("aria-label", v ?? "");
                break;
            case "SOURCE":
                node.setAttribute("src", v ?? "");
                break;
            case "PLACEHOLDER":
                node.setAttribute("placeholder", v ?? "");
                break;
            case "VALUE":
                node.value = v ?? "";
                break;
            case "CHECKED":
                node.checked = v;
                if (node.tagName === "BUTTON") node.setAttribute("aria-pressed", String(v));
                break;
            default:
                throw problem("UnknownOp");
        }
    }
    function dispatchEvent(target, name, detail) {
        const EventType = target.ownerDocument?.defaultView?.CustomEvent ?? globalThis.CustomEvent;
        target.dispatchEvent?.(EventType ? new EventType(name, {
            bubbles: true,
            detail
        }) : {
            type: name,
            detail,
            bubbles: true
        });
    }
    function sheetSubmitField(button) {
        if (!button.classList?.contains("ewe-submit-button") || !button.closest?.(".ewe-bottom-sheet")) return null;
        const fields = Array.from(button.parentElement.children).filter((node)=>node.tagName === "INPUT" && !node.disabled && !node.hidden && node.hasAttribute("primal:onsubmit"));
        return fields.length === 1 ? fields[0] : null;
    }
    function wireBottomSheet(sheet, options) {
        function submit(input) {
            if (input && !input.disabled) {
                const EventType = sheet.ownerDocument.defaultView.Event;
                input.dispatchEvent(new EventType("submit", {
                    cancelable: true
                }));
            }
        }
        sheet.addEventListener("keydown", (event)=>{
            const input = event.target;
            if (event.key !== "Enter" || event.isComposing || input.closest(".ewe-bottom-sheet") !== sheet) return;
            if (input.tagName !== "INPUT" || !input.hasAttribute("primal:onsubmit")) return;
            event.preventDefault();
            if (!event.repeat) submit(input);
        }, options);
        sheet.addEventListener("click", (event)=>{
            const button = event.target.closest?.('button.ewe-submit-button');
            if (!button || button.closest(".ewe-bottom-sheet") !== sheet || button.hasAttribute("primal:onclick")) return;
            submit(sheetSubmitField(button));
        }, options);
    }
    class FrameApplicator {
        constructor(deps){
            this.deps = deps;
            this.document = deps.document;
            this.registry = deps.registry;
            this.elementListeners = new WeakMap();
            this.bindings = [];
            this.layers = new Set();
            this.containerEvents = new WeakMap();
            this.maxFrameBytes = deps.maxFrameBytes ?? defaultFrameBytes;
            this.gapTimeoutMs = deps.gapTimeoutMs ?? 1000;
            this.maxPendingFrames = deps.maxPendingFrames ?? 128;
            this.timer = deps.timer ?? {
                set: setTimeout,
                clear: clearTimeout
            };
            this.schedule = deps.schedule ?? ((run)=>{
                if (this.document.visibilityState === "hidden" || !globalThis.requestAnimationFrame) queueMicrotask(run);
                else globalThis.requestAnimationFrame(run);
            });
            this.stats = {
                frames: 0,
                rows: 0,
                reports: 0,
                copiedColumns: 0,
                zeroCopyColumns: 0
            };
            this.defaultView = {
                registry: deps.registry,
                cursors: new Map(),
                gaps: new Map()
            };
            this.viewStates = new Map();
            this.loaders = new Map();
            this.queue = [];
            this.scheduled = false;
            this.cssInstalled = false;
            this.nodeIds = new WeakMap();
            this.origins = new WeakMap();
            this.views = new Map();
            this.sharedKinds = new WeakMap();
            this.drawingTargets = [];
            this.drawingJobs = new Map();
        }
        bindDrawing(canvas, nodeId) {
            const graphics = this.deps.graphics;
            if (!graphics || !canvas.isConnected) return;
            const name = canvas.id || String(nodeId);
            const host = this.document.defaultView;
            let surface, removed = false, observer;
            const enqueue = (work)=>{
                const previous = this.drawingJobs.get(name) ?? Promise.resolve();
                const job = previous.then(work).catch((error)=>this.report("Graphics", {
                        node: nodeId,
                        detail: error.message
                    }));
                this.drawingJobs.set(name, job);
                job.then(()=>{
                    if (this.drawingJobs.get(name) === job) this.drawingJobs.delete(name);
                });
            };
            const resize = ()=>{
                if (removed) return;
                const rect = canvas.getBoundingClientRect();
                const scale = host.devicePixelRatio || 1;
                const width = Math.round(rect.width * scale), height = Math.round(rect.height * scale);
                if (canvas.width === width && canvas.height === height && surface) return;
                if (canvas.width !== width) canvas.width = width;
                if (canvas.height !== height) canvas.height = height;
                enqueue(async ()=>{
                    if (removed) return;
                    if (surface) await surface.resize(width, height);
                    else surface = await graphics.attach(name, canvas);
                });
            };
            resize();
            if (host.ResizeObserver) {
                observer = new host.ResizeObserver(resize);
                observer.observe(canvas);
            }
            this.registerUnmount(canvas, ()=>{
                removed = true;
                observer?.disconnect();
                enqueue(async ()=>{
                    if (surface) await surface.remove();
                });
            });
        }
        viewOwner(mount) {
            for(let node = mount; node; node = node.parentNode){
                if (this.viewStates.has(node)) return node;
            }
            return this.deps.viewMount ?? this;
        }
        viewState(mount) {
            return this.viewStates.get(this.viewOwner(mount)) ?? this.defaultView;
        }
        cursor(stream, mount) {
            return this.viewState(mount).cursors.get(stream);
        }
        setCursor(stream, sequence, mount) {
            const { gaps, cursors } = this.viewState(mount);
            const gap = gaps.get(stream);
            if (gap) this.timer.clear(gap.timer);
            gaps.delete(stream);
            cursors.set(stream, sequence);
        }
        registerUnmount(node, hook) {
            Patcher.registerUnmount(node, hook);
        }
        report(code, fields = {}) {
            this.stats.reports++;
            const report = {
                code,
                ...fields
            };
            if (this.deps.onReport) this.deps.onReport(report);
            else console.error("Frame operation failed", report);
        }
        receive(input, origin) {
            try {
                const bytes = asBytes(input);
                if (bytes.length > this.maxFrameBytes) {
                    this.report("TooLarge");
                    return false;
                }
                if (!isFrame(bytes)) throw problem("BadFrame");
                const frame = mod1.Frame.getRootAsFrame(new CheckedBuffer(bytes));
                if (frame.kindType() !== mod1.FrameKind.FrameMessage) throw problem("BadFrame");
                const msg = frame.kind(new mod1.FrameMessage());
                if (!msg) throw problem("BadFrame");
                const count = msg.messagesLength();
                if (count) {
                    const bb = msg.bb;
                    bb.check(bb.__vector(msg.bb_pos + bb.__offset(msg.bb_pos, 6)), count * 4);
                }
                const seq = frame.sequenceId();
                const stream = frame.streamId();
                if (origin) {
                    this.origins.set(msg, origin);
                    origin.observeStream?.(stream);
                }
                if (seq === 0n || msg.ack() !== 0n) this.enqueue(msg);
                else this.order(stream, seq, msg);
                return true;
            } catch (error) {
                this.report("BadFrame", {
                    detail: error.message
                });
                return false;
            }
        }
        order(stream, seq, msg) {
            const owner = this.viewOwner(this.origins.get(msg)?.viewMount);
            const { cursors, gaps } = this.viewState(owner);
            const cursor = cursors.get(stream);
            if (cursor !== undefined && seq <= cursor) return;
            let gap = gaps.get(stream);
            if (cursor !== undefined && seq > cursor + 1n) {
                if (!gap) {
                    gap = {
                        held: new Map(),
                        timer: this.timer.set(()=>{
                            gaps.delete(stream);
                            this.deps.onGap?.({
                                streamId: stream,
                                ...this.viewStates.has(owner) ? {
                                    view: owner
                                } : {},
                                resumeFrom: cursors.get(stream)
                            });
                        }, this.gapTimeoutMs)
                    };
                    gaps.set(stream, gap);
                }
                if (!gap.held.has(seq) && gap.held.size >= this.maxPendingFrames) {
                    this.timer.clear(gap.timer);
                    gaps.delete(stream);
                    this.deps.onGap?.({
                        streamId: stream,
                        ...this.viewStates.has(owner) ? {
                            view: owner
                        } : {},
                        resumeFrom: cursor
                    });
                } else gap.held.set(seq, msg);
                return;
            }
            this.enqueue(msg);
            while(gap?.held.has(seq + 1n)){
                seq++;
                this.enqueue(gap.held.get(seq));
                gap.held.delete(seq);
            }
            cursors.set(stream, seq);
            if (gap && !gap.held.size) {
                this.timer.clear(gap.timer);
                gaps.delete(stream);
            }
        }
        enqueue(msg) {
            this.queue.push(msg);
            if (!this.scheduled) {
                this.scheduled = true;
                this.schedule(()=>this.flush());
            }
        }
        flush() {
            this.scheduled = false;
            for (const msg of this.queue.splice(0)){
                const previousOrigin = this.origin;
                this.origin = this.origins.get(msg);
                if (this.origin?.active && !this.origin.active()) {
                    this.origin = previousOrigin;
                    continue;
                }
                const previousRegistry = this.registry;
                this.registry = this.viewState(this.origin?.viewMount).registry;
                this.stats.frames++;
                for(let i = 0; i < msg.messagesLength(); i++){
                    try {
                        const m = msg.messages(i);
                        if (m.operation()) this.applyUnit(m.operation(), i);
                        if (m.data()) this.applyData(m.data());
                        if (m.control()) this.applyControl(m.control());
                    } catch (e) {
                        this.report(e.report?.code ?? "BadFrame", {
                            unitIndex: i,
                            ...e.report ?? {},
                            detail: e.message
                        });
                    }
                }
                for (const { node, id, index, name, args, shared, registry: nodeRegistry } of this.bindings.splice(0)){
                    if (nodeRegistry.get(id) !== node) continue;
                    const registry = this.deps.behaviours ?? mod2;
                    try {
                        const applied = shared ? registry.applyBehaviour(node, name, args) : registry.applyWebBehaviour(node, name, args);
                        if (!applied) this.report("UnknownBehaviour", {
                            unitIndex: index,
                            node: id,
                            name: String(name)
                        });
                    } catch (error) {
                        this.report("BadFrame", {
                            unitIndex: index,
                            node: id,
                            detail: error.message
                        });
                    }
                }
                for (const layer of this.layers){
                    if (!layer.isConnected) continue;
                    const hidden = layer.closest("[hidden]") !== null;
                    if (layer.tagName === "DIALOG") {
                        if (hidden && layer.open) layer.close();
                        else if (!hidden && !layer.open) layer.showModal();
                    } else {
                        const open = layer.matches(":popover-open");
                        if (hidden && open) layer.hidePopover();
                        else if (!hidden && !open) layer.showPopover();
                    }
                }
                for (const [canvas, nodeId] of this.drawingTargets.splice(0))this.bindDrawing(canvas, nodeId);
                this.registry = previousRegistry;
                this.origin = previousOrigin;
                this.deps.onApplied?.();
            }
        }
        node(id, staged = false) {
            const n = this.registry.get(id);
            if (n) return n;
            if (staged && this.registry.pending.has(id)) return this.registry.expectOrStaged(id);
            throw problem("UnknownNode", {
                node: id
            });
        }
        applyUnit(op, index) {
            const read = (key, enumType, handler)=>{
                const id = op[key + "Type"]();
                if (!id) return false;
                const name = enumType[id];
                if (!mod1[name]) throw problem("UnknownOp", {
                    field: key
                });
                const reader = op[key](new mod1[name]());
                if (!reader) throw problem("BadFrame");
                handler.call(this, name, reader, index);
                this.stats.rows++;
                return true;
            };
            const ui = read("ui", mod1.UiOperation, this.applyUiReader);
            const web = read("web", mod1.WebOperation, this.applyWebReader);
            if (op.columnar()) this.applyColumnar(op.columnar(), index);
            if (op.mobileType()) this.report("UnsupportedPlatformOp", {
                unitIndex: index,
                field: "mobile"
            });
            else if (!ui && !web && !op.columnar()) this.report("UnknownOp", {
                unitIndex: index
            });
        }
        applyUiReader(name, reader, index) {
            if (name === "ReplaceSubtree") {
                const root = this.node(reader.nodeId());
                for (const child of Array.from(root.childNodes)){
                    this.discard(child);
                    root.removeChild(child);
                }
                this.applyNested(reader, index);
            } else if (name === "Place") this.place(reader, index);
            else this.applyUi(name, reader.unpack(), index);
        }
        applyNested(reader, index) {
            for(let i = 0; i < reader.operationsLength(); i++)this.applyUnit(reader.operations(i), index);
        }
        discard(node) {
            if (this.layers.delete(node)) {
                if (node.tagName === "DIALOG" && node.open) node.close();
                else if (node.hasAttribute("popover") && node.matches(":popover-open")) node.hidePopover();
            }
            this.containerEvents.get(node)?.abort();
            this.containerEvents.delete(node);
            for (const child of Array.from(node.childNodes ?? []))this.discard(child);
            Patcher.releaseScope(node);
            mod2.disposeBehaviours(node);
            this.deps.onUnmount?.(node);
            (this.deps.dispatcher ?? this.deps.runtime?.dispatcher)?.off?.(node);
            const id = this.nodeIds.get(node) ?? node.getAttribute?.("primal-id");
            if (id != null && this.registry.get(Number(id)) === node) {
                this.registry.unregister(Number(id));
            }
            for (const [name, handler] of this.elementListeners.get(node) ?? []){
                node.removeEventListener(name, handler);
            }
            this.elementListeners.delete(node);
            this.nodeIds.delete(node);
        }
        selectorQuery(selector) {
            const text = selector.text;
            const escape = this.document.defaultView?.CSS?.escape ?? globalThis.CSS?.escape;
            if (selector.kind === mod1.SelectorKind.Id) return "#" + escape(text);
            if (selector.kind === mod1.SelectorKind.Class) return "." + escape(text);
            if (selector.kind === mod1.SelectorKind.Query) return text;
            throw problem("UnknownOp");
        }
        resolve(selector) {
            if (!selector) return null;
            if (selector.kind === mod1.SelectorKind.NodeId) return this.registry.get(selector.nodeId);
            if (selector.kind === mod1.SelectorKind.Id) return this.document.getElementById(selector.text);
            return this.document.querySelector(this.selectorQuery(selector));
        }
        registerView(name, root) {
            if (!/^[A-Za-z_][A-Za-z0-9_-]*$/.test(name) || !root || this.views.has(name) || [
                ...this.views.values()
            ].includes(root)) throw problem("DuplicateView", {
                detail: name
            });
            const primary = root === this.deps.viewMount || !this.deps.viewMount && this.views.size === 0;
            const state = primary ? this.defaultView : {
                registry: new NodeRegistry(),
                cursors: new Map(),
                gaps: new Map()
            };
            this.viewStates.set(root, state);
            this.views.set(name, root);
            return ()=>{
                if (this.views.get(name) !== root) return;
                const previous = this.registry;
                this.registry = state.registry;
                try {
                    for (const child of Array.from(root.childNodes)){
                        this.discard(child);
                        child.remove();
                    }
                } finally{
                    this.registry = previous;
                    for (const gap of state.gaps.values())this.timer.clear(gap.timer);
                    state.gaps.clear();
                    state.cursors.clear();
                    this.loaders.get(root)?.close();
                    this.loaders.delete(root);
                    this.views.delete(name);
                    this.viewStates.delete(root);
                }
            };
        }
        registerLoader(root, load, options = {}) {
            if (this.loaders.get(root)?.active) throw new Error("View already has a loader");
            const recovery = new ViewRecovery(load, (bytes, expired)=>{
                if (!isFrame(bytes) || bytes.byteLength > this.maxFrameBytes) throw new Error("Invalid view snapshot");
                const frame = mod1.Frame.getRootAsFrame(new CheckedBuffer(asBytes(bytes)));
                const body = frame.kindType() === mod1.FrameKind.FrameMessage && frame.kind(new mod1.FrameMessage());
                if (!body || !body.messagesLength()) throw new Error("Expected a full view");
                for(let index = 1; index < body.messagesLength(); index++){
                    const message = body.messages(index);
                    if (!message.data() || message.operation() || message.control()) throw new Error("Expected snapshot data");
                }
                const operation = body.messages(0).operation();
                const place = operation?.uiType() === mod1.UiOperation.Place && operation.ui(new mod1.Place());
                if (!place || place.kind() !== mod1.PlacementKind.FullView) throw new Error("Expected a full view");
                const doc = this.deps.snapshotDocument?.() ?? this.document.implementation.createHTMLDocument("");
                const detached = doc.createElement("main");
                doc.body.appendChild(detached);
                const issues = [];
                const probe = new FrameApplicator({
                    document: doc,
                    registry: new NodeRegistry().seedDocument(doc),
                    viewMount: detached,
                    schedule: (run)=>run(),
                    maxFrameBytes: this.maxFrameBytes,
                    onReport: (error)=>issues.push(error)
                });
                if (!probe.receive(bytes) || issues.length) throw new Error("Invalid snapshot content");
                const cursors = new Map();
                for(let i = 0; i < place.operationsLength(); i++){
                    const op = place.operations(i);
                    if (op.uiType() !== mod1.UiOperation.LiveMount) continue;
                    const mount = op.ui(new mod1.LiveMount());
                    if (mount.kind() !== mod1.LiveKind.Stream || mount.transport() === mod1.LiveTransport.Local || mount.transport() === mod1.LiveTransport.Sse && mount.sharing() === mod1.ConnectionSharing.Shared) continue;
                    if (cursors.has(mount.channelId()) && cursors.get(mount.channelId()) !== mount.resumeFrom()) throw new Error("Conflicting stream cursors");
                    cursors.set(mount.channelId(), mount.resumeFrom());
                }
                for (const stream of expired)if (!cursors.has(stream)) throw new Error("Snapshot is missing the expired stream");
                const state = this.viewState(root);
                for (const gap of state.gaps.values())this.timer.clear(gap.timer);
                state.gaps.clear();
                state.cursors.clear();
                for (const [stream, cursor] of cursors)this.setCursor(stream, cursor, root);
                if (!this.receive(bytes, {
                    viewMount: root
                })) throw new Error("Invalid view snapshot");
                this.flush();
            }, options);
            this.loaders.set(root, recovery);
            return recovery;
        }
        registerElement(name, tag, node) {
            const valid = /^[A-Za-z_][A-Za-z0-9_-]*$/;
            if (!valid.test(name) || !valid.test(tag) || tag.toLowerCase() === "view") throw problem("InvalidName");
            let root = node;
            while(root.parentNode && ![
                ...this.views.values(),
                this.deps.viewMount
            ].includes(root))root = root.parentNode;
            const other = this.namedElements(root).get(name);
            if (other && other !== node) throw problem("DuplicateName", {
                detail: name
            });
            node.setAttribute("id", name);
            this.sharedKinds.set(node, tag);
        }
        receivingView(supplied = this.origin?.viewMount) {
            if (supplied) {
                for(let node = supplied; node; node = node.parentNode){
                    if (node === this.deps.viewMount || [
                        ...this.views.values()
                    ].includes(node)) return node;
                }
                return supplied;
            }
            const roots = new Set(this.views.values());
            if (this.deps.viewMount) roots.add(this.deps.viewMount);
            if (roots.size > 1) throw problem("AmbiguousView");
            return roots.values().next().value;
        }
        namedElements(root) {
            const names = new Map(), pending = [
                root
            ];
            const otherViews = new Set(this.views.values());
            while(pending.length){
                const node = pending.pop();
                if (!node || node !== root && otherViews.has(node)) continue;
                const name = node.getAttribute?.("id");
                if (name) {
                    if (names.has(name)) throw problem("DuplicateName", {
                        detail: name
                    });
                    names.set(name, node);
                }
                for (const child of node.children ?? [])pending.push(child);
            }
            return names;
        }
        placementTarget(selector, parent, scope) {
            if (!selector) return null;
            const kinds = mod1.SelectorKind;
            if (selector.kind === kinds.NodeId) {
                const node = this.registry.get(selector.nodeId);
                return node && (!parent || node.parentNode === parent) ? node : null;
            }
            if (selector.kind === kinds.View) return this.views.get(selector.text);
            if (selector.kind === kinds.Receiving) return this.receivingView();
            if (selector.kind === kinds.Id || selector.kind === kinds.TypedId) {
                const typed = selector.kind === kinds.TypedId;
                const parts = typed ? selector.text?.split("#") : [
                    null,
                    selector.text
                ];
                if (parts?.length !== 2 || typed && !/^[A-Za-z_][A-Za-z0-9_-]*$/.test(parts[0])) throw problem("InvalidName");
                const [tag, name] = parts;
                if (!/^[A-Za-z_][A-Za-z0-9_-]*$/.test(name)) throw problem("InvalidName");
                const root = scope ?? this.receivingView() ?? this.document.body;
                const node = this.namedElements(root).get(name);
                if (!node || parent && node.parentNode !== parent) return null;
                if (typed) {
                    const compact = (text)=>text?.replace(/[-_]/g, "").toLowerCase();
                    const expected = compact(tag) === "sheet" ? "bottomsheet" : compact(tag);
                    if (compact(this.sharedKinds.get(node) ?? node.localName) !== expected) throw problem("WrongTargetKind", {
                        detail: selector.text
                    });
                }
                return node;
            }
            const query = this.selectorQuery(selector);
            const matches = parent ? Array.from(parent.children).filter((node)=>node.matches(query)) : this.document.querySelectorAll(query);
            return matches.length === 1 ? matches[0] : null;
        }
        validatePlacementNames(reader, scope, outgoing) {
            const names = this.namedElements(scope);
            for (const [name, node] of names)if (outgoing?.some((root)=>root === node || root.contains?.(node))) names.delete(name);
            const check = (name)=>{
                if (!name || !/^[A-Za-z_][A-Za-z0-9_-]*$/.test(name)) throw problem("InvalidName");
                if (names.has(name)) throw problem("DuplicateName", {
                    detail: name
                });
                names.set(name, true);
            };
            for(let i = 0; i < reader.operationsLength(); i++){
                const unit = reader.operations(i);
                if (unit.uiType() === mod1.UiOperation.SetProp) {
                    const prop = unit.ui(new mod1.SetProp());
                    if (prop.prop() === mod1.PropId.ID) check(prop.value(new mod1.TextValue())?.v());
                }
                const batch = unit.columnar();
                if (!batch) continue;
                const bytes = batch.blobBytesArray() ?? new Uint8Array();
                for(let row = 0; row < batch.opCodesLength(); row++){
                    if (batch.opCodes(row) !== mod1.UiOperation.SetProp || (batch.argA(row) & 0xffff) !== mod1.PropId.ID) continue;
                    const start = batch.blobAOff(row), end = batch.blobAOff(row + 1);
                    if (start > end || end > bytes.length) throw problem("BadFrame");
                    check(utf8.decode(bytes.subarray(start, end)));
                }
            }
        }
        place(reader, index) {
            const target = reader.target();
            const root = target?.kind() === mod1.SelectorKind.View ? this.views.get(target.text()) : this.origin?.viewMount;
            const previous = this.registry;
            this.registry = this.viewState(root).registry;
            try {
                this.applyPlacement(reader, index);
            } finally{
                this.registry = previous;
            }
        }
        applyPlacement(reader, index) {
            const kind = mod1.PlacementKind[reader.kind()];
            if (kind === "FullView") {
                const mount = this.receivingView();
                if (!mount || mount.isConnected === false) throw problem("UnknownNode", {
                    detail: "Missing receiving view mount"
                });
                this.validatePlacementNames(reader, mount, Array.from(mount.childNodes));
                for (const node of Array.from(mount.childNodes)){
                    this.discard(node);
                    node.remove();
                }
                this.applyNested(reader, index);
                mount.appendChild(this.node(reader.rootNode(), true));
                return;
            }
            const selector = reader.target()?.unpack();
            const scope = selector?.kind === mod1.SelectorKind.View ? this.views.get(selector.text) : this.receivingView() ?? this.document.body;
            const target = this.placementTarget(selector, null, scope);
            const missing = (value)=>[
                    mod1.SelectorKind.Id,
                    mod1.SelectorKind.View,
                    mod1.SelectorKind.TypedId,
                    mod1.SelectorKind.Receiving
                ].includes(value?.kind) ? "MissingTarget" : "UnknownNode";
            if (!target) throw problem(missing(selector), {
                detail: "Placement needs one target"
            });
            const childSelector = reader.child()?.unpack();
            const child = childSelector && this.placementTarget(childSelector, target, scope);
            if (childSelector && (!child || child.parentNode !== target)) throw problem(missing(childSelector), {
                detail: "Placement child missing"
            });
            this.validatePlacementNames(reader, scope, kind === "Swap" ? [
                target
            ] : []);
            if (kind === "Remove") {
                this.discard(target);
                target.remove();
                return;
            }
            if (kind === "Swap") this.discard(target);
            this.applyNested(reader, index);
            const root = this.node(reader.rootNode(), true);
            switch(kind){
                case "Append":
                    target.appendChild(root);
                    break;
                case "Swap":
                    target.replaceWith(root);
                    break;
                case "Before":
                    target.insertBefore(root, child ?? target.firstChild);
                    break;
                case "After":
                    target.insertBefore(root, child?.nextSibling ?? null);
                    break;
                default:
                    throw problem("UnknownOp");
            }
        }
        applyUi(name, m, index) {
            const d = this.document;
            switch(name){
                case "CreateNode":
                    {
                        if (!this.cssInstalled) {
                            const style = d.createElement("style");
                            style.setAttribute("data-ewe-core", "");
                            style.textContent = CORE_CSS;
                            d.head.appendChild(style);
                            this.cssInstalled = true;
                        }
                        const kind = WEB_KINDS[m.kind];
                        if (!kind) this.report("UnknownKind", {
                            unitIndex: index,
                            node: m.nodeId
                        });
                        const el = d.createElement(kind?.tag ?? "div");
                        this.sharedKinds.set(el, mod1.NodeKind[m.kind]);
                        el.className = kind?.cls ?? "ewe-unknown";
                        if (m.kind === mod1.NodeKind.DRAWING_SURFACE) this.drawingTargets.push([
                            el,
                            m.nodeId
                        ]);
                        for (const [attr, value] of kind?.attrs ?? [])el.setAttribute(attr, value);
                        el.setAttribute("primal-id", String(m.nodeId));
                        if (m.kind === mod1.NodeKind.OVERLAY && el.showPopover) {
                            el.setAttribute("popover", "manual");
                            this.layers.add(el);
                        }
                        if ([
                            mod1.NodeKind.MODAL,
                            mod1.NodeKind.CONFIRMATION_DIALOG,
                            mod1.NodeKind.BOTTOM_SHEET
                        ].includes(m.kind)) {
                            const Controller = d.defaultView?.AbortController ?? globalThis.AbortController;
                            const controller = new Controller();
                            this.containerEvents.set(el, controller);
                            const options = {
                                signal: controller.signal
                            };
                            if (m.kind !== mod1.NodeKind.BOTTOM_SHEET) {
                                this.layers.add(el);
                                el.addEventListener("cancel", (event)=>{
                                    event.preventDefault();
                                    const cancel = Array.from(el.querySelectorAll('.ewe-cancel-button')).find((node)=>node.closest("dialog") === el && !node.disabled && !node.closest("[hidden]") && node.hasAttribute("primal:onclick"));
                                    cancel?.click();
                                }, options);
                            } else {
                                wireBottomSheet(el, options);
                            }
                        }
                        if (m.sharing !== mod1.ConnectionSharing.TransportDefault) {
                            el.setAttribute("data-connection-shared", String(m.sharing === mod1.ConnectionSharing.Shared));
                        }
                        if (m.keepAlive) el.setAttribute("data-connection-keep-alive", "true");
                        this.origin?.onMount?.(m.nodeId, el);
                        this.registry.register(m.nodeId, el);
                        break;
                    }
                case "SetText":
                    {
                        const node = this.node(m.nodeId);
                        for (const child of Array.from(node.childNodes))this.discard(child);
                        node.textContent = m.text ?? "";
                        break;
                    }
                case "SetProp":
                    applyProp(this.node(m.nodeId), m.prop, m.valueType, m.value);
                    break;
                case "AppendChild":
                    this.node(m.parentId).appendChild(this.node(m.childId, true));
                    break;
                case "InsertBefore":
                    {
                        const parent = this.node(m.parentId);
                        const before = this.node(m.beforeId);
                        parent.insertBefore(this.node(m.childId, true), before);
                        break;
                    }
                case "Remove":
                    {
                        const node = this.node(m.nodeId);
                        this.discard(node);
                        node.remove();
                        this.registry.unregister(m.nodeId);
                        break;
                    }
                case "BindAction":
                case "UnbindAction":
                    {
                        const el = this.node(m.nodeId), event = WEB_EVENTS[m.event];
                        if (!event) throw problem("UnknownOp");
                        const attr = "primal:on" + event;
                        const dispatcher = this.deps.dispatcher ?? this.deps.runtime?.dispatcher;
                        if (name === "BindAction") {
                            el.setAttribute(attr, m.action);
                            dispatcher?.scanAndWire?.(el);
                        } else {
                            el.removeAttribute(attr);
                            dispatcher?.off?.(el, event);
                        }
                        break;
                    }
                case "AttachState":
                    this.node(m.rootNode).setAttribute("primal::state", base64urlEncode(m.state ?? []));
                    break;
                case "UiBehaviour":
                    this.bindings.push({
                        registry: this.registry,
                        node: this.node(m.nodeId),
                        id: m.nodeId,
                        index,
                        name: m.behaviour,
                        args: m.args,
                        shared: true
                    });
                    break;
                case "LiveMount":
                    {
                        const el = d.createElement(m.kind === mod1.LiveKind.Stream ? "mount-stream" : "mount-data");
                        const transport = [
                            "local",
                            "sse",
                            "websocket",
                            "fetch"
                        ][m.transport];
                        for (const [key, value] of Object.entries({
                            api: m.endpoint,
                            transport,
                            protocol: "frame",
                            "data-channel-id": m.channelId,
                            "data-resume-from": m.resumeFrom,
                            "primal-id": m.nodeId
                        }))el.setAttribute(key, String(value));
                        if (m.sharing !== mod1.ConnectionSharing.TransportDefault) {
                            el.setAttribute("data-connection-shared", String(m.sharing === mod1.ConnectionSharing.Shared));
                        }
                        if (m.keepAlive) el.setAttribute("data-connection-keep-alive", "true");
                        this.origin?.onMount?.(m.nodeId, el);
                        this.registry.register(m.nodeId, el);
                        break;
                    }
                default:
                    throw problem("UnknownOp", {
                        field: "ui"
                    });
            }
        }
        applyWebReader(name, reader, index) {
            const m = reader.unpack();
            const d = this.document;
            const id = m.nodeId;
            const resolveName = (id, custom, names)=>{
                const value = id ? names[id - 1] : custom;
                if (!value) throw problem("BadFrame");
                return value;
            };
            const property = ()=>resolveName(m.nameId, m.customName, ATTR_NAMES);
            switch(name){
                case "WebElement":
                    {
                        const el = d.createElement(resolveName(m.tagId, m.customTag, TAG_NAMES));
                        if (m.className) el.className = m.className;
                        this.origin?.onMount?.(id, el);
                        this.nodeIds.set(el, id);
                        this.registry.stage(id, el);
                        break;
                    }
                case "WebText":
                    {
                        const node = d.createTextNode(m.content ?? "");
                        this.origin?.onMount?.(id, node);
                        this.nodeIds.set(node, id);
                        this.registry.stage(id, node);
                        break;
                    }
                case "WebRegisterNode":
                    this.registry.applyRegister(id, d);
                    break;
                case "WebUnregisterNode":
                    this.registry.unregister(id);
                    break;
                case "WebReplaceNode":
                    {
                        const old = this.node(m.oldId), next = this.node(m.newId, true);
                        this.discard(old);
                        old.replaceWith(next);
                        this.registry.unregister(m.oldId);
                        this.registry.register(m.newId, next);
                        break;
                    }
                case "WebDetachChild":
                    this.node(m.parentId).removeChild(this.node(m.childId));
                    break;
                case "WebSetAttribute":
                case "WebRemoveAttribute":
                    {
                        const attr = resolveName(m.attrId, m.customAttr, ATTR_NAMES);
                        if (attr.toLowerCase() === "primal::state") throw problem("BadFrame");
                        if (name === "WebSetAttribute") this.node(id).setAttribute(attr, m.value ?? "");
                        else this.node(id).removeAttribute(attr);
                        break;
                    }
                case "HtmlMorph":
                    {
                        const target = this.resolve(m.target);
                        if (!target) throw problem("Morph");
                        if (m.action === mod1.MorphAction.ReplaceChildren && this.deps.morph && d.createRange) {
                            const previous = Array.from(target.querySelectorAll("*"));
                            this.deps.morph(target, d.createRange().createContextualFragment(m.content), d);
                            const removed = new Set(previous.filter((n)=>!target.contains(n)));
                            for (const n of removed)if (!removed.has(n.parentNode)) this.discard(n);
                        } else {
                            switch(m.action){
                                case mod1.MorphAction.ReplaceChildren:
                                    for (const child of Array.from(target.childNodes ?? []))this.discard(child);
                                    target.innerHTML = m.content;
                                    break;
                                case mod1.MorphAction.ReplaceElement:
                                    target.insertAdjacentHTML("afterend", m.content);
                                    this.discard(target);
                                    target.remove();
                                    break;
                                case mod1.MorphAction.InsertBefore:
                                    target.insertAdjacentHTML("beforebegin", m.content);
                                    break;
                                case mod1.MorphAction.InsertAfter:
                                    target.insertAdjacentHTML("afterend", m.content);
                                    break;
                                case mod1.MorphAction.AppendSibling:
                                    (target.parentNode ?? target.parent)?.insertAdjacentHTML("beforeend", m.content);
                                    break;
                                default:
                                    throw problem("Morph");
                            }
                        }
                        break;
                    }
                case "ScriptNode":
                    {
                        const el = d.createElement("script");
                        el.setAttribute("primal:script", "");
                        el.setAttribute("scoped", "");
                        el.type = "application/primal-script";
                        el.textContent = m.body;
                        const parent = this.node(m.parentId);
                        parent.appendChild(el);
                        this.deps.hydrate?.(parent, d);
                        break;
                    }
                case "WebBehaviour":
                    {
                        this.bindings.push({
                            registry: this.registry,
                            node: this.node(id),
                            id,
                            index,
                            name: m.custom,
                            args: m.args,
                            shared: false
                        });
                        break;
                    }
                case "WebProperty":
                    {
                        let value = m.value;
                        try {
                            value = JSON.parse(value);
                        } catch  {}
                        this.node(id)[property()] = value;
                        break;
                    }
                case "WebStyle":
                    this.node(id).style[property()] = m.value;
                    break;
                case "WebAddClass":
                    this.node(id).classList.add(m.className);
                    break;
                case "WebRemoveClass":
                    this.node(id).classList.remove(m.className);
                    break;
                case "WebAddListener":
                case "WebRemoveListener":
                    {
                        const node = this.node(id), event = property();
                        let listeners = this.elementListeners.get(node);
                        if (!listeners) this.elementListeners.set(node, listeners = new Map());
                        if (name === "WebAddListener" && !listeners.has(event)) {
                            const deliver = this.origin?.onEvent ?? this.deps.onEvent;
                            const handler = (value)=>deliver?.(event, id, value, node);
                            listeners.set(event, handler);
                            node.addEventListener(event, handler);
                        } else if (name === "WebRemoveListener" && listeners.has(event)) {
                            node.removeEventListener(event, listeners.get(event));
                            listeners.delete(event);
                        }
                        break;
                    }
                default:
                    throw problem("UnknownOp", {
                        field: "web"
                    });
            }
        }
        applyColumnar(batch, index) {
            const codes = batch.opCodesArray() ?? [];
            const cols = [
                "nodeIds",
                "argA",
                "argB",
                "blobAOff",
                "blobBOff"
            ].map((name, i)=>vector(batch, name, 6 + 2 * i, Uint32Array, this.stats));
            const [nodes, args, values, starts, second] = cols;
            const bytes = batch.blobBytesArray() ?? new Uint8Array();
            if (cols.some((v, i)=>v.length !== codes.length + Number(i >= 3)) || starts[0] !== 0 || starts.at(-1) !== bytes.length || second[0] !== 0 && second[0] !== bytes.length) throw problem("BadFrame");
            const bits = new DataView(new ArrayBuffer(4));
            for(let i = 0; i < codes.length; i++){
                try {
                    if (starts[i] > starts[i + 1] || starts[i + 1] > bytes.length || second[i] !== second[i + 1]) throw problem("BadFrame");
                    const blob = bytes.subarray(starts[i], starts[i + 1]);
                    const name = mod1.UiOperation[codes[i]];
                    const nodeId = nodes[i], a = args[i], b = values[i];
                    const m = {
                        nodeId
                    };
                    switch(name){
                        case "CreateNode":
                            m.kind = a;
                            break;
                        case "SetText":
                            m.text = utf8.decode(blob);
                            break;
                        case "SetProp":
                            {
                                m.prop = a & 0xffff;
                                const kind = PROP_KIND[m.prop];
                                m.valueType = mod1.PropValue[kind];
                                bits.setUint32(0, b, true);
                                switch(kind){
                                    case "Text":
                                        m.value = {
                                            v: utf8.decode(blob)
                                        };
                                        break;
                                    case "Length":
                                        m.value = {
                                            v: {
                                                value: bits.getFloat32(0, true),
                                                unit: a >>> 16
                                            }
                                        };
                                        break;
                                    case "ColorToken":
                                    case "FontToken":
                                        m.value = {
                                            id: b
                                        };
                                        break;
                                    case "Bool":
                                        m.value = {
                                            v: Boolean(b)
                                        };
                                        break;
                                    case "Axis":
                                    case "Alignment":
                                        m.value = {
                                            v: b
                                        };
                                        break;
                                    default:
                                        throw problem("BadFrame");
                                }
                                break;
                            }
                        case "InsertBefore":
                        case "AppendChild":
                            Object.assign(m, {
                                parentId: nodeId,
                                childId: a,
                                beforeId: b
                            });
                            break;
                        case "Remove":
                            break;
                        case "BindAction":
                            m.action = utf8.decode(blob);
                            m.event = a;
                            break;
                        case "UnbindAction":
                            m.event = a;
                            break;
                        case "AttachState":
                            m.rootNode = nodeId;
                            m.state = blob;
                            break;
                        default:
                            throw problem("UnknownOp", {
                                field: "columnar"
                            });
                    }
                    this.applyUi(name, m, index);
                    this.stats.rows++;
                } catch (e) {
                    this.report(e.report?.code ?? "BadFrame", {
                        unitIndex: index,
                        ...e.report ?? {},
                        row: i
                    });
                }
            }
        }
        applyData(data) {
            const table = data.table();
            let detail;
            if (table) {
                const columns = [];
                const types = {
                    I32: [
                        mod1.I32s,
                        Int32Array
                    ],
                    I64: [
                        mod1.I64s,
                        BigInt64Array
                    ],
                    F64: [
                        mod1.F64s,
                        Float64Array
                    ],
                    Bool: [
                        mod1.Bools,
                        Uint8Array
                    ],
                    Utf8: [
                        mod1.Strings
                    ]
                };
                for(let i = 0; i < table.columnsLength(); i++){
                    const col = table.columns(i);
                    const type = types[mod1.ColumnData[col.dataType()]];
                    if (!type) throw problem("BadFrame");
                    const values = col.data(new type[0]());
                    const items = type[1] ? vector(values, "values", 4, type[1]) : Array.from({
                        length: values.valuesLength()
                    }, (_, j)=>values.values(j));
                    columns.push({
                        name: col.name(),
                        data: items,
                        validity: col.validityArray()
                    });
                }
                detail = {
                    kind: "table",
                    table: {
                        rows: table.rows(),
                        columns
                    }
                };
            } else {
                const blob = data.arrow() ?? data.msgpack();
                if (!blob) throw problem("BadFrame");
                detail = {
                    kind: data.arrow() ? "arrow-ipc" : "messagepack",
                    bytes: blob.bytesArray()
                };
            }
            const target = data.targetNode() ? this.node(data.targetNode()) : this.receivingView();
            dispatchEvent(target || this.document, "ewe:data", detail);
        }
        applyControl(control) {
            if (control.caps()) this.deps.onCaps?.(control.caps().unpack());
            for (const [field, code] of [
                [
                    "stateExpired",
                    "StateExpired"
                ],
                [
                    "updateRequired",
                    "UpdateRequired"
                ]
            ]){
                if (control[field]()) this.deps.onError?.({
                    ...control[field]().unpack(),
                    code
                });
            }
            if (control.error()) {
                const error = control.error().unpack();
                const detail = {
                    ...error,
                    code: mod1.ErrorCode[error.code]
                };
                this.deps.onError?.(detail);
                dispatchEvent(this.document, "ewe:error", detail);
            }
        }
    }
    function pushed(bytes) {
        const frame = Frame.getRootAsFrame(new ByteBuffer(bytes));
        const body = frame.kindType() === FrameKind.FrameMessage && frame.kind(new FrameMessage());
        const control = body && body.messagesLength() ? body.messages(0)?.control() : null;
        return {
            frame,
            body,
            control
        };
    }
    function integer(element, attribute) {
        const raw = element.getAttribute(attribute);
        if (raw === null) return undefined;
        if (!/^\d+$/.test(raw)) throw new TypeError("Expected an unsigned integer for " + attribute);
        const n = BigInt(raw);
        if (n > 0xffffffffffffffffn) throw new RangeError(attribute + " exceeds u64");
        return n;
    }
    function bindLiveMounts(runtime, frames) {
        const streams = new Map();
        const connections = frames.deps.connections ?? frameConnections();
        runtime.connections = connections;
        const previousGap = frames.deps.onGap;
        frames.deps.onGap = (gap)=>{
            for (const binding of streams.get(gap.streamId) ?? []){
                if (!gap.view || gap.view === binding.owner) binding.resume(gap.resumeFrom);
            }
            previousGap?.(gap);
        };
        runtime.onMountConnect = (element, streaming)=>{
            if (element.getAttribute("protocol") !== "frame") return null;
            let stream = integer(element, "data-channel-id");
            const owner = frames.viewOwner(element);
            const recovery = frames.loaders.get(owner);
            const generation = recovery?.generation;
            const current = ()=>active && (!recovery || recovery.accepts(generation));
            const expired = ()=>{
                if (!current()) return;
                if (recovery) {
                    Promise.resolve(recovery.expire(stream)).catch(()=>{});
                } else if (frames.deps.requestSnapshot) Promise.resolve(frames.deps.requestSnapshot({
                    element,
                    stream
                })).catch((error)=>frames.deps.onError?.(error));
                else frames.deps.onError?.({
                    code: "ReplayExpired",
                    stream
                });
            };
            const start = integer(element, "data-resume-from") ?? 0n;
            const local = element.getAttribute("transport") === "local";
            let active = true;
            let resumeOnConnection;
            function attach(id) {
                stream = id;
                if (start && frames.cursor(id, owner) === undefined) frames.setCursor(id, start, owner);
                if (streaming && !local) {
                    let mounts = streams.get(id);
                    if (!mounts) streams.set(id, mounts = new Set());
                    mounts.add(binding);
                }
            }
            const binding = {
                local,
                owner,
                expired,
                managed ({ api, data, transport }) {
                    const kind = element.getAttribute("transport") ?? "sse";
                    const sharing = element.getAttribute("data-connection-shared");
                    const shared = sharing === null ? kind === "websocket" || kind === "ws" : sharing === "true";
                    const keepAlive = element.getAttribute("data-connection-keep-alive") === "true";
                    const socket = kind === "websocket" || kind === "ws";
                    const adapter = socket ? "websocket" : kind === "http" ? "chunked" : kind;
                    if (!connections.supports(adapter)) return null;
                    const negotiate = Boolean(frames.deps.deliverySession) && (socket || [
                        "sse",
                        "chunked"
                    ].includes(adapter));
                    if (!shared && !keepAlive && !negotiate) return null;
                    if (sharing === null && !keepAlive && !negotiate && !frames.deps.connections) return null;
                    const endpoint = shared || negotiate ? api : binding.url(api);
                    const subscription = connections.nextTag();
                    const hello = new ClientHelloT(VOCABULARY_VERSION, Platform.Web, MountSurface.Web, 0n, true, subscription);
                    const greeting = requestBytes(0n, new FrameRequestT(null, hello));
                    const report = (error)=>frames.deps.onError?.({
                            code: "Transport",
                            detail: String(error)
                        });
                    let handle, session, wasConnected = false, pendingCursor = recovery && !(shared && adapter === "sse") ? start : start || null;
                    const resume = async (cursor)=>{
                        if (stream === undefined || !session) {
                            pendingCursor = cursor;
                            return;
                        }
                        const request = new FrameRequestT(null, null, new ResumeSubscriptionT(session, subscription, stream, cursor));
                        const bytes = requestBytes(session, request);
                        if (socket) await handle.send(bytes);
                        else {
                            const response = await (frames.deps.fetchFn ?? fetch)(endpoint, {
                                method: "POST",
                                body: bytes,
                                headers: {
                                    ...frames.deps.connectionHeaders,
                                    "content-type": "application/primal-frame"
                                }
                            });
                            if (response.status === 410) {
                                expired();
                                return;
                            }
                            if (!response.ok) throw new Error("Resume request failed with HTTP " + response.status);
                        }
                    };
                    const context = (bytes)=>{
                        const caps = pushed(bytes).control?.caps();
                        if (!caps?.subscriptionResume() || socket && caps.requestTag() !== subscription) return;
                        session = caps.streamId();
                        if (wasConnected && !shared && stream !== undefined) pendingCursor ??= frames.cursor(stream, owner) ?? start;
                        wasConnected = true;
                        if (pendingCursor !== null) {
                            const cursor = pendingCursor;
                            pendingCursor = null;
                            resume(cursor).catch(report);
                        }
                    };
                    if (negotiate) resumeOnConnection = (cursor)=>resume(cursor).catch(report);
                    handle = connections.acquire({
                        transport: adapter,
                        endpoint,
                        shared,
                        keepAlive,
                        base: element.ownerDocument?.baseURI ?? globalThis.location?.href,
                        headers: negotiate && !socket ? {
                            ...frames.deps.connectionHeaders,
                            Accept: adapter === "chunked" ? "application/primal-frame" : "text/event-stream",
                            "X-Ewe-Hello": base64urlEncode(greeting)
                        } : frames.deps.connectionHeaders,
                        isContext: !socket && negotiate ? (bytes)=>Boolean(pushed(bytes).control?.caps()) : undefined,
                        channel: element._transportOverride,
                        settings: element._transportOverride ?? (element.hasAttribute?.("data") ? data : frames.deps.connectionSettings),
                        authority: frames.deps.authority,
                        data: element.getAttribute("data") === null ? undefined : data
                    }, {
                        owner,
                        onOpen () {
                            if (socket && negotiate) queueMicrotask(()=>{
                                if (active) handle.send(greeting).catch(report);
                            });
                        },
                        onContext: context,
                        accept (bytes) {
                            const { frame, body, control } = pushed(bytes);
                            if (!body) return false;
                            if (control?.caps()) return socket && control.caps().requestTag() === subscription;
                            return (stream === undefined || stream === frame.streamId()) && !body.ack() && (!body.subscriptionId() || body.subscriptionId() === subscription);
                        },
                        receive (bytes) {
                            const { control } = pushed(bytes);
                            if (control?.caps()) {
                                context(bytes);
                                return;
                            }
                            if (control?.replayExpired()) {
                                expired();
                                return;
                            }
                            binding.receive({
                                type: "frame",
                                frames: [
                                    bytes
                                ]
                            });
                        },
                        onError (cause) {
                            session = null;
                            if ((socket || !shared) && stream !== undefined) pendingCursor = frames.cursor(stream, owner) ?? start;
                            report(cause);
                        }
                    });
                    return handle;
                },
                url (api) {
                    const cursor = stream === undefined ? undefined : frames.cursor(stream, owner) ?? start;
                    if (stream === undefined || !cursor) return api;
                    const url = new URL(api, element.ownerDocument?.baseURI ?? globalThis.location.href);
                    url.searchParams.set("stream_id", stream.toString());
                    url.searchParams.set("resume_from", cursor.toString());
                    return url.href;
                },
                receive (result) {
                    if (!current()) return;
                    if (result.type !== "frame") throw new Error("Frame mount received another protocol");
                    for (const bytes of result.frames){
                        frames.receive(bytes, {
                            active: current,
                            get viewMount () {
                                return active && frameMountTarget(element, element.getAttribute("target"), element.ownerDocument);
                            },
                            observeStream (id) {
                                if (stream === undefined) attach(id);
                                else if (stream !== id) throw new Error("Unexpected stream identifier");
                            }
                        });
                    }
                },
                resume (cursor) {
                    if (!current()) return;
                    element.setAttribute("data-resume-from", String(cursor));
                    if (resumeOnConnection) resumeOnConnection(cursor);
                    else element._restartFrameStream?.();
                },
                close () {
                    active = false;
                    const mounts = streams.get(stream);
                    mounts?.delete(binding);
                    if (mounts?.size === 0) streams.delete(stream);
                }
            };
            if (stream !== undefined) attach(stream);
            return binding;
        };
    }
    function installFrameRuntime(wasmUi, options = {}) {
        const doc = options.document ?? globalThis.document;
        const runtime = wasmUi.Patcher.runtime;
        if (runtime.frames?.document === doc) return runtime.frames;
        if (runtime.document && runtime.document !== doc) runtime.dispatcher = null;
        runtime.frames = new FrameApplicator({
            document: doc,
            registry: wasmUi.Patcher.ensureRegistry(doc),
            morph: wasmUi.MorphDom.morph,
            hydrate: wasmUi.Hydrator.hydrate,
            runtime,
            onApplied: wasmUi.Patcher.markApplied,
            ...options
        });
        bindLiveMounts(runtime, runtime.frames);
        wasmUi.Hydrator.behaviourMarkers = mod2.hydrateMarkers;
        if (doc.body) mod2.hydrateMarkers(doc.body);
        wasmUi.ProtocolHandler.frameHook = {
            isFrame,
            splitFrames,
            readFrames,
            readViewResponse,
            maxFrameBytes: runtime.frames.maxFrameBytes
        };
        return runtime.frames;
    }
    globalThis.EweFrames = {
        installFrameRuntime
    };
    function registerFrameApp(runtime, options = {}) {
        const frames = installFrameRuntime(mod, options);
        mod.registerWasmApp(runtime, options);
        mod.registerWebComponents();
        return frames;
    }
    globalThis.FoundationUiBootstrap = {
        registerFrameApp
    };
    (function() {
        'use strict';
        var PLATFORM_SCHEMES = [
            'ewe',
            'foundation',
            'platform'
        ];
        var PREFIX = 'http://';
        function isPlatformScheme(href) {
            if (!href) return false;
            for(var i = 0; i < PLATFORM_SCHEMES.length; i++){
                var prefix = PLATFORM_SCHEMES[i] + '://';
                if (href.indexOf(prefix) === 0) return PLATFORM_SCHEMES[i];
            }
            return null;
        }
        function toWorkaroundUrl(href) {
            var scheme = isPlatformScheme(href);
            if (!scheme) return null;
            return href.replace(scheme + '://', PREFIX + scheme + '.');
        }
        function isAndroid() {
            return /android/i.test(navigator.userAgent);
        }
        if (!isAndroid()) return;
        document.addEventListener('click', function(e) {
            var a = e.target.closest('a');
            if (!a) return;
            var href = a.getAttribute('href') || a.href;
            var newUrl = toWorkaroundUrl(href);
            if (!newUrl) return;
            e.preventDefault();
            e.stopImmediatePropagation();
            location.href = newUrl;
        }, true);
        var _origAssign = location.assign;
        var _origReplace = location.replace;
        var _origSetHref = Object.getOwnPropertyDescriptor(Location.prototype, 'href');
        function wrap(fn) {
            return function(url) {
                var rewritten = toWorkaroundUrl(url);
                if (rewritten) {
                    url = rewritten;
                }
                return fn.call(this, url);
            };
        }
        try {
            location.assign = wrap(_origAssign);
        } catch (_) {
            try {
                Object.defineProperty(location, 'assign', {
                    value: wrap(_origAssign),
                    writable: true,
                    configurable: true
                });
            } catch (__) {}
        }
        try {
            location.replace = wrap(_origReplace);
        } catch (_) {
            try {
                Object.defineProperty(location, 'replace', {
                    value: wrap(_origReplace),
                    writable: true,
                    configurable: true
                });
            } catch (__) {}
        }
        if (_origSetHref && _origSetHref.set) {
            var _set = _origSetHref.set;
            Object.defineProperty(location, 'href', {
                get: _origSetHref.get,
                set: function(url) {
                    var rewritten = toWorkaroundUrl(url);
                    _set.call(this, rewritten || url);
                },
                configurable: true,
                enumerable: true
            });
        }
        var _origOpen = window.open;
        window.open = function(url) {
            var rewritten = toWorkaroundUrl(url);
            if (rewritten) {
                arguments[0] = rewritten;
            }
            return _origOpen.apply(this, arguments);
        };
    })();
    return {};
})();
