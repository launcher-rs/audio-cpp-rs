#include "capi.h"

#include "engine/framework/core/backend.h"
#include "engine/framework/io/json.h"
#include "engine/framework/runtime/model.h"
#include "engine/framework/runtime/registry.h"
#include "engine/framework/runtime/session.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <string>
#include <vector>

using engine::runtime::ModelRegistry;
using engine::runtime::ModelLoadRequest;
using engine::runtime::ILoadedVoiceModel;
using engine::runtime::IVoiceTaskSession;
using engine::runtime::IOfflineVoiceTaskSession;
using engine::runtime::IStreamingVoiceTaskSession;
using engine::runtime::TaskSpec;
using engine::runtime::SessionOptions;
using engine::runtime::TaskRequest;
using engine::runtime::TaskResult;
using engine::runtime::StreamEvent;
using engine::runtime::VoiceTaskKind;
using engine::runtime::RunMode;

namespace json = engine::io::json;

/* ------------------------------------------------------------------ */
/* 不透明结构体                                                         */
/* ------------------------------------------------------------------ */

struct audiocpp_registry {
    ModelRegistry storage;
};

struct audiocpp_model {
    std::unique_ptr<ILoadedVoiceModel> storage;
};

struct EventSink {
    audiocpp_stream_event_cb cb = nullptr;
    void * user_data = nullptr;
};

struct audiocpp_session {
    std::unique_ptr<IVoiceTaskSession> storage;
    EventSink sink;
    // 借用字符串缓存：task_kind / run_mode 由 to_string() 产生临时对象，
    // 不能直接返回其 c_str()（悬垂指针），因此建会话时固化到成员里。
    std::string family;
    std::string task_kind;
    std::string run_mode;
};

/* ------------------------------------------------------------------ */
/* 错误处理                                                             */
/* ------------------------------------------------------------------ */

static thread_local std::string tls_last_error;

static void set_last_error(const std::string & msg) {
    tls_last_error = msg;
}

const char * audiocpp_last_error(void) {
    return tls_last_error.empty() ? nullptr : tls_last_error.c_str();
}

static char * dup_string(const std::string & s) {
    char * out = static_cast<char *>(std::malloc(s.size() + 1));
    if (out != nullptr) {
        std::memcpy(out, s.c_str(), s.size());
        out[s.size()] = '\0';
    }
    return out;
}

void audiocpp_free_string(char * s) {
    std::free(s);
}

/* ------------------------------------------------------------------ */
/* 小工具函数                                                           */
/* ------------------------------------------------------------------ */

static json::Value dump_time_span(const engine::runtime::TimeSpan & span) {
    json::Value::Object obj;
    obj.emplace("start_sample", json::Value::make_number(static_cast<double>(span.start_sample)));
    obj.emplace("end_sample", json::Value::make_number(static_cast<double>(span.end_sample)));
    return json::Value::make_object(std::move(obj));
}

static json::Value dump_speech_segment(const engine::runtime::SpeechSegment & seg) {
    json::Value::Object obj;
    obj.emplace("span", dump_time_span(seg.span));
    obj.emplace("confidence", json::Value::make_number(seg.confidence));
    obj.emplace("text", json::Value::make_string(seg.text));
    return json::Value::make_object(std::move(obj));
}

static json::Value dump_audio_buffer(const engine::runtime::AudioBuffer & audio) {
    json::Value::Object obj;
    obj.emplace("sample_rate", json::Value::make_number(audio.sample_rate));
    obj.emplace("channels", json::Value::make_number(audio.channels));
    obj.emplace("sample_count", json::Value::make_number(static_cast<double>(audio.samples.size())));
    return json::Value::make_object(std::move(obj));
}

static json::Value dump_task_result(const TaskResult & result) {
    json::Value::Object obj;

    json::Value::Array segments;
    segments.reserve(result.speech_segments.size());
    for (const auto & seg : result.speech_segments) {
        segments.push_back(dump_speech_segment(seg));
    }
    obj.emplace("speech_segments", json::Value::make_array(std::move(segments)));

    if (result.text_output.has_value()) {
        json::Value::Object text;
        text.emplace("text", json::Value::make_string(result.text_output->text));
        text.emplace("language", json::Value::make_string(result.text_output->language));
        obj.emplace("text_output", json::Value::make_object(std::move(text)));
    }
    if (result.audio_output.has_value()) {
        obj.emplace("audio_output", dump_audio_buffer(*result.audio_output));
    }

    json::Value::Array named_audio;
    named_audio.reserve(result.named_audio_outputs.size());
    for (const auto & named : result.named_audio_outputs) {
        json::Value::Object item;
        item.emplace("id", json::Value::make_string(named.id));
        item.emplace("audio", dump_audio_buffer(named.audio));
        json::Value::Object meta;
        for (const auto & [k, v] : named.meta) {
            meta.emplace(k, json::Value::make_string(v));
        }
        item.emplace("meta", json::Value::make_object(std::move(meta)));
        named_audio.push_back(json::Value::make_object(std::move(item)));
    }
    obj.emplace("named_audio_outputs", json::Value::make_array(std::move(named_audio)));

    return json::Value::make_object(std::move(obj));
}

static json::Value dump_stream_event(const StreamEvent & event) {
    json::Value::Object obj;
    json::Value::Array activity;
    for (const auto & ev : event.voice_activity) {
        json::Value::Object item;
        switch (ev.kind) {
            case engine::runtime::VoiceActivityEvent::Kind::SpeechStart:
                item.emplace("kind", json::Value::make_string("speech_start"));
                break;
            case engine::runtime::VoiceActivityEvent::Kind::SpeechEnd:
                item.emplace("kind", json::Value::make_string("speech_end"));
                break;
            default:
                item.emplace("kind", json::Value::make_string("speech_segment"));
                break;
        }
        item.emplace("sample", json::Value::make_number(static_cast<double>(ev.sample)));
        item.emplace("probability", json::Value::make_number(ev.probability));
        if (ev.segment.has_value()) {
            item.emplace("segment", dump_speech_segment(*ev.segment));
        }
        activity.push_back(json::Value::make_object(std::move(item)));
    }
    obj.emplace("voice_activity", json::Value::make_array(std::move(activity)));
    if (event.partial_text.has_value()) {
        json::Value::Object text;
        text.emplace("text", json::Value::make_string(event.partial_text->text));
        text.emplace("language", json::Value::make_string(event.partial_text->language));
        obj.emplace("partial_text", json::Value::make_object(std::move(text)));
    }
    if (event.audio_output.has_value()) {
        obj.emplace("audio_output", dump_audio_buffer(*event.audio_output));
    }
    obj.emplace("is_final", json::Value::make_bool(event.is_final));
    return json::Value::make_object(std::move(obj));
}

static json::Value dump_capabilities(const engine::runtime::CapabilitySet & caps) {
    json::Value::Object obj;
    json::Value::Array tasks;
    for (const auto & task : caps.supported_tasks) {
        json::Value::Object item;
        item.emplace("task", json::Value::make_string(engine::runtime::to_string(task.task)));
        json::Value::Array modes;
        for (const auto mode : task.modes) {
            modes.push_back(json::Value::make_string(engine::runtime::to_string(mode)));
        }
        item.emplace("modes", json::Value::make_array(std::move(modes)));
        tasks.push_back(json::Value::make_object(std::move(item)));
    }
    obj.emplace("supported_tasks", json::Value::make_array(std::move(tasks)));
    json::Value::Array languages;
    for (const auto & lang : caps.languages) {
        languages.push_back(json::Value::make_string(lang));
    }
    obj.emplace("languages", json::Value::make_array(std::move(languages)));
    obj.emplace("supports_speaker_reference", json::Value::make_bool(caps.supports_speaker_reference));
    obj.emplace("supports_style_condition", json::Value::make_bool(caps.supports_style_condition));
    obj.emplace("supports_timestamps", json::Value::make_bool(caps.supports_timestamps));
    return json::Value::make_object(std::move(obj));
}

static json::Value dump_metadata(const engine::runtime::ModelMetadata & meta) {
    json::Value::Object obj;
    obj.emplace("family", json::Value::make_string(meta.family));
    obj.emplace("variant", json::Value::make_string(meta.variant));
    obj.emplace("description", json::Value::make_string(meta.description));
    json::Value::Array configs;
    for (const auto & c : meta.config_candidates) {
        configs.push_back(json::Value::make_string(c));
    }
    obj.emplace("config_candidates", json::Value::make_array(std::move(configs)));
    json::Value::Array weights;
    for (const auto & w : meta.weight_candidates) {
        weights.push_back(json::Value::make_string(w));
    }
    obj.emplace("weight_candidates", json::Value::make_array(std::move(weights)));
    return json::Value::make_object(std::move(obj));
}

/* ------------------------------------------------------------------ */
/* WAV 加载（极简 RIFF/WAVE PCM 读取器）                                */
/* ------------------------------------------------------------------ */

int audiocpp_audio_load_wav(const char * path, int * sample_rate, int * channels,
                            size_t * count, float ** samples) {
    if (samples != nullptr) {
        *samples = nullptr;
    }
    if (count != nullptr) {
        *count = 0;
    }
    FILE * f = std::fopen(path, "rb");
    if (f == nullptr) {
        set_last_error("could not open wav file");
        return -1;
    }
    struct RiffHeader {
        char id[4];
        uint32_t size;
        char wave[4];
    } riff;
    if (std::fread(&riff, sizeof(riff), 1, f) != 1 ||
        std::memcmp(riff.id, "RIFF", 4) != 0 || std::memcmp(riff.wave, "WAVE", 4) != 0) {
        set_last_error("not a RIFF/WAVE file");
        std::fclose(f);
        return -1;
    }
    uint16_t audio_format = 0;
    uint16_t num_channels = 0;
    uint32_t sample_rate32 = 0;
    uint16_t bits_per_sample = 0;
    std::vector<uint8_t> data;
    bool have_fmt = false;

    while (true) {
        char chunk_id[4];
        uint32_t chunk_size = 0;
        if (std::fread(chunk_id, 4, 1, f) != 1) {
            break;
        }
        if (std::fread(&chunk_size, 4, 1, f) != 1) {
            break;
        }
        if (std::memcmp(chunk_id, "fmt ", 4) == 0) {
            uint16_t format_tag = 0;
            uint16_t n_channels = 0;
            uint32_t n_sample_rate = 0;
            uint32_t n_byte_rate = 0;
            uint16_t n_block_align = 0;
            uint16_t n_bits = 0;
            if (std::fread(&format_tag, 2, 1, f) != 1 ||
                std::fread(&n_channels, 2, 1, f) != 1 ||
                std::fread(&n_sample_rate, 4, 1, f) != 1 ||
                std::fread(&n_byte_rate, 4, 1, f) != 1 ||
                std::fread(&n_block_align, 2, 1, f) != 1 ||
                std::fread(&n_bits, 2, 1, f) != 1) {
                break;
            }
            audio_format = format_tag;
            num_channels = n_channels;
            sample_rate32 = n_sample_rate;
            bits_per_sample = n_bits;
            have_fmt = true;
            if (chunk_size > 16) {
                std::fseek(f, static_cast<long>(chunk_size) - 16L, SEEK_CUR);
            }
        } else if (std::memcmp(chunk_id, "data", 4) == 0) {
            data.resize(chunk_size);
            if (chunk_size != 0 && std::fread(data.data(), chunk_size, 1, f) != 1) {
                break;
            }
        } else {
            if (std::fseek(f, static_cast<long>(chunk_size), SEEK_CUR) != 0) {
                break;
            }
        }
        if (chunk_size % 2 != 0) {
            std::fseek(f, 1, SEEK_CUR);
        }
    }
    std::fclose(f);

    if (!have_fmt || data.empty()) {
        set_last_error("missing fmt or data chunk");
        return -1;
    }
    const size_t bytes_per_sample = bits_per_sample / 8;
    if (bytes_per_sample == 0 || num_channels == 0) {
        set_last_error("bad wav format");
        return -1;
    }
    const size_t n = data.size() / bytes_per_sample;
    auto * out = static_cast<float *>(std::malloc(n * sizeof(float)));
    if (out == nullptr) {
        set_last_error("out of memory");
        return -1;
    }
    if (audio_format == 3) {
        for (size_t i = 0; i < n; ++i) {
            float v;
            std::memcpy(&v, data.data() + i * bytes_per_sample, sizeof(float));
            out[i] = v;
        }
    } else if (audio_format == 1 && bits_per_sample == 16) {
        const int16_t * src = reinterpret_cast<const int16_t *>(data.data());
        for (size_t i = 0; i < n; ++i) {
            out[i] = static_cast<float>(src[i]) / 32768.0f;
        }
    } else if (audio_format == 1 && bits_per_sample == 32) {
        const int32_t * src = reinterpret_cast<const int32_t *>(data.data());
        for (size_t i = 0; i < n; ++i) {
            out[i] = static_cast<float>(src[i]) / 2147483648.0f;
        }
    } else {
        set_last_error("unsupported wav sample format");
        std::free(out);
        return -1;
    }
    if (sample_rate != nullptr) {
        *sample_rate = static_cast<int>(sample_rate32);
    }
    if (channels != nullptr) {
        *channels = num_channels;
    }
    if (count != nullptr) {
        *count = n;
    }
    if (samples != nullptr) {
        *samples = out;
    }
    return 0;
}

void audiocpp_audio_free(float * samples) {
    std::free(samples);
}

/* ------------------------------------------------------------------ */
/* 请求解析                                                             */
/* ------------------------------------------------------------------ */

static void fill_options_from_json(std::unordered_map<std::string, std::string> & dst,
                                   const json::Value * options) {
    if (options == nullptr || !options->is_object()) {
        return;
    }
    for (const auto & [key, value] : options->as_object()) {
        if (value.is_string()) {
            dst.emplace(key, value.as_string());
        } else {
            dst.emplace(key, json::stringify(value));
        }
    }
}

static TaskRequest parse_task_request(const json::Value & root) {
    TaskRequest request;
    fill_options_from_json(request.options, root.find("options"));

    if (const auto * text = root.find("text"); text != nullptr && text->is_string()) {
        engine::runtime::Transcript transcript;
        transcript.text = text->as_string();
        if (const auto * lang = root.find("language"); lang != nullptr && lang->is_string()) {
            transcript.language = lang->as_string();
        }
        request.text_input = std::move(transcript);
    }

    if (const auto * audio = root.find("audio"); audio != nullptr && audio->is_object()) {
        engine::runtime::AudioBuffer buf;
        buf.sample_rate = json::optional_i32(*audio, "sample_rate", 0);
        buf.channels = json::optional_i32(*audio, "channels", 1);
        if (const auto * samples = audio->find("samples"); samples != nullptr && samples->is_array()) {
            buf.samples = json::number_array_as<float>(*samples);
        }
        if (!buf.samples.empty()) {
            request.audio_input = std::move(buf);
        }
    }

    if (const auto * audio_path = root.find("audio_path"); audio_path != nullptr && audio_path->is_string()) {
        int sr = 0, ch = 0;
        size_t n = 0;
        float * samples = nullptr;
        if (audiocpp_audio_load_wav(audio_path->as_string().c_str(), &sr, &ch, &n, &samples) == 0) {
            engine::runtime::AudioBuffer buf;
            buf.sample_rate = sr;
            buf.channels = ch;
            buf.samples.assign(samples, samples + n);
            std::free(samples);
            request.audio_input = std::move(buf);
        }
    }
    return request;
}

/* ------------------------------------------------------------------ */
/* 注册表                                                               */
/* ------------------------------------------------------------------ */

audiocpp_registry * audiocpp_registry_default(void) {
    try {
        auto reg = engine::runtime::make_default_registry();
        return new audiocpp_registry{std::move(reg)};
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return nullptr;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_registry_default");
        return nullptr;
    }
}

int audiocpp_registry_families_json(const audiocpp_registry * reg, char ** out_json) {
    if (out_json == nullptr || reg == nullptr) {
        set_last_error("null out_json or registry");
        return -1;
    }
    *out_json = nullptr;
    try {
        json::Value::Array arr;
        for (const auto & family : reg->storage.families()) {
            arr.push_back(json::Value::make_string(family));
        }
        *out_json = dup_string(json::stringify(json::Value::make_array(std::move(arr))));
        return 0;
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return -1;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_registry_families_json");
        return -1;
    }
}

int audiocpp_registry_loaders_json(const audiocpp_registry * reg, char ** out_json) {
    if (out_json == nullptr || reg == nullptr) {
        set_last_error("null out_json or registry");
        return -1;
    }
    *out_json = nullptr;
    try {
        json::Value::Object root;
        json::Value::Array arr;
        for (const auto & adv : reg->storage.advertise_loaders()) {
            json::Value::Object item;
            item.emplace("family", json::Value::make_string(adv.family));
            item.emplace("capabilities", dump_capabilities(adv.capabilities));
            item.emplace("instructions_policy", json::Value::make_string(adv.instructions_policy));
            json::Value::Array endpoints;
            for (const auto & e : adv.api_endpoints) {
                endpoints.push_back(json::Value::make_string(e));
            }
            item.emplace("api_endpoints", json::Value::make_array(std::move(endpoints)));
            arr.push_back(json::Value::make_object(std::move(item)));
        }
        root.emplace("loaders", json::Value::make_array(std::move(arr)));
        *out_json = dup_string(json::stringify(json::Value::make_object(std::move(root))));
        return 0;
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return -1;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_registry_loaders_json");
        return -1;
    }
}

int audiocpp_registry_devices_json(char ** out_json) {
    if (out_json == nullptr) {
        set_last_error("null out_json");
        return -1;
    }
    *out_json = nullptr;
    try {
        json::Value::Array arr;
        for (const auto & dev : engine::core::list_backend_devices()) {
            json::Value::Object item;
            item.emplace("backend", json::Value::make_string(dev.backend));
            item.emplace("index", json::Value::make_number(dev.index));
            item.emplace("name", json::Value::make_string(dev.name));
            item.emplace("type", json::Value::make_string(dev.type));
            arr.push_back(json::Value::make_object(std::move(item)));
        }
        *out_json = dup_string(json::stringify(json::Value::make_array(std::move(arr))));
        return 0;
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return -1;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_registry_devices_json");
        return -1;
    }
}

audiocpp_model * audiocpp_registry_load(const audiocpp_registry * reg,
                                        const char * model_path,
                                        const char * family_hint,
                                        const char * load_options_json) {
    if (reg == nullptr || model_path == nullptr) {
        set_last_error("null registry or model_path");
        return nullptr;
    }
    try {
        ModelLoadRequest request;
        request.model_path = model_path;
        if (family_hint != nullptr && *family_hint != '\0') {
            request.family_hint = std::string(family_hint);
        }
        if (load_options_json != nullptr && *load_options_json != '\0') {
            const json::Value root = json::parse(load_options_json);
            if (const auto * config_id = root.find("config_id"); config_id != nullptr && config_id->is_string()) {
                request.config_id = config_id->as_string();
            }
            if (const auto * weight_id = root.find("weight_id"); weight_id != nullptr && weight_id->is_string()) {
                request.weight_id = weight_id->as_string();
            }
            if (const auto * spec = root.find("model_spec_override");
                spec != nullptr && spec->is_string()) {
                request.model_spec_override = std::filesystem::path(spec->as_string());
            }
            fill_options_from_json(request.options, root.find("options"));
        }
        auto model = reg->storage.load(request);
        return new audiocpp_model{std::move(model)};
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return nullptr;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_registry_load");
        return nullptr;
    }
}

void audiocpp_registry_free(audiocpp_registry * reg) {
    delete reg;
}

/* ------------------------------------------------------------------ */
/* 模型                                                                 */
/* ------------------------------------------------------------------ */

int audiocpp_model_metadata_json(const audiocpp_model * model, char ** out_json) {
    if (out_json == nullptr || model == nullptr) {
        set_last_error("null out_json or model");
        return -1;
    }
    *out_json = nullptr;
    try {
        *out_json = dup_string(json::stringify(dump_metadata(model->storage->metadata())));
        return 0;
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return -1;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_model_metadata_json");
        return -1;
    }
}

int audiocpp_model_capabilities_json(const audiocpp_model * model, char ** out_json) {
    if (out_json == nullptr || model == nullptr) {
        set_last_error("null out_json or model");
        return -1;
    }
    *out_json = nullptr;
    try {
        *out_json = dup_string(json::stringify(dump_capabilities(model->storage->capabilities())));
        return 0;
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return -1;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_model_capabilities_json");
        return -1;
    }
}

audiocpp_session * audiocpp_model_create_task_session(const audiocpp_model * model,
                                                      const char * task,
                                                      const char * mode,
                                                      const char * backend,
                                                      int device,
                                                      int threads,
                                                      const char * session_options_json) {
    if (model == nullptr || task == nullptr) {
        set_last_error("null model or task");
        return nullptr;
    }
    try {
        const auto task_kind = engine::runtime::parse_voice_task_kind(task);
        const RunMode run_mode = (mode != nullptr && std::string(mode) == "streaming")
                                     ? RunMode::Streaming
                                     : RunMode::Offline;
        TaskSpec spec{task_kind, run_mode};

        SessionOptions opts;
        std::string backend_name = backend != nullptr ? backend : "cpu";
        if (backend_name == "cuda") {
            opts.backend.type = engine::core::BackendType::Cuda;
        } else if (backend_name == "hip" || backend_name == "rocm") {
            opts.backend.type = engine::core::BackendType::Hip;
        } else if (backend_name == "vulkan") {
            opts.backend.type = engine::core::BackendType::Vulkan;
        } else if (backend_name == "metal") {
            opts.backend.type = engine::core::BackendType::Metal;
        } else if (backend_name == "best") {
            opts.backend.type = engine::core::BackendType::BestAvailable;
        } else {
            opts.backend.type = engine::core::BackendType::Cpu;
        }
        opts.backend.device = device;
        opts.backend.threads = threads > 0 ? threads : 1;
        if (session_options_json != nullptr && *session_options_json != '\0') {
            const json::Value root = json::parse(session_options_json);
            fill_options_from_json(opts.options, root.find("options"));
        }

        auto session = model->storage->create_task_session(spec, opts);
        auto * wrapper = new audiocpp_session{std::move(session)};
        wrapper->family = wrapper->storage->family();
        wrapper->task_kind = engine::runtime::to_string(wrapper->storage->task_kind());
        wrapper->run_mode = engine::runtime::to_string(wrapper->storage->run_mode());
        return wrapper;
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return nullptr;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_model_create_task_session");
        return nullptr;
    }
}

void audiocpp_model_free(audiocpp_model * model) {
    delete model;
}

/* ------------------------------------------------------------------ */
/* 会话                                                                 */
/* ------------------------------------------------------------------ */

void audiocpp_session_free(audiocpp_session * session) {
    delete session;
}

const char * audiocpp_session_family(audiocpp_session * session) {
    return session != nullptr ? session->family.c_str() : nullptr;
}

const char * audiocpp_session_task_kind(audiocpp_session * session) {
    return session != nullptr ? session->task_kind.c_str() : nullptr;
}

const char * audiocpp_session_run_mode(audiocpp_session * session) {
    return session != nullptr ? session->run_mode.c_str() : nullptr;
}

int audiocpp_session_streaming_policy_json(audiocpp_session * session, char ** out_json) {
    if (out_json == nullptr || session == nullptr) {
        set_last_error("null out_json or session");
        return -1;
    }
    *out_json = nullptr;
    try {
        auto * streaming = dynamic_cast<IStreamingVoiceTaskSession *>(session->storage.get());
        if (streaming == nullptr) {
            set_last_error("session does not support streaming");
            return -1;
        }
        const auto policy = streaming->streaming_policy();
        json::Value::Object obj;
        const char * input = "none";
        if (policy.input == engine::runtime::StreamingInputKind::AudioChunks) {
            input = "audio_chunks";
        }
        obj.emplace("input", json::Value::make_string(input));
        const char * output = "final_result";
        if (policy.output == engine::runtime::StreamingOutputKind::PullEvents) {
            output = "pull_events";
        }
        obj.emplace("output", json::Value::make_string(output));
        obj.emplace("preferred_audio_chunk_samples",
                    json::Value::make_number(static_cast<double>(policy.preferred_audio_chunk_samples)));
        obj.emplace("preferred_audio_chunk_seconds", json::Value::make_number(policy.preferred_audio_chunk_seconds));
        *out_json = dup_string(json::stringify(json::Value::make_object(std::move(obj))));
        return 0;
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return -1;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_session_streaming_policy_json");
        return -1;
    }
}

int audiocpp_session_prepare(audiocpp_session * session, const char * request_json) {
    if (session == nullptr || request_json == nullptr) {
        set_last_error("null session or request_json");
        return -1;
    }
    try {
        const json::Value root = json::parse(request_json);
        const auto request = parse_task_request(root);
        session->storage->prepare(engine::runtime::build_preparation_request(request));
        return 0;
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return -1;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_session_prepare");
        return -1;
    }
}

int audiocpp_session_run_offline(audiocpp_session * session,
                                 const char * request_json,
                                 char ** out_json) {
    if (out_json != nullptr) {
        *out_json = nullptr;
    }
    if (session == nullptr || request_json == nullptr) {
        set_last_error("null session or request_json");
        return -1;
    }
    try {
        auto * offline = dynamic_cast<IOfflineVoiceTaskSession *>(session->storage.get());
        if (offline == nullptr) {
            set_last_error("session does not support offline execution");
            return -1;
        }
        const json::Value root = json::parse(request_json);
        const auto request = parse_task_request(root);
        session->storage->prepare(engine::runtime::build_preparation_request(request));
        const TaskResult result = offline->run(request);
        if (out_json != nullptr) {
            *out_json = dup_string(json::stringify(dump_task_result(result)));
        }
        return 0;
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return -1;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_session_run_offline");
        return -1;
    }
}

void audiocpp_session_set_event_sink(audiocpp_session * session,
                                     audiocpp_stream_event_cb cb,
                                     void * user_data) {
    if (session == nullptr) {
        return;
    }
    session->sink.cb = cb;
    session->sink.user_data = user_data;
    auto * streaming = dynamic_cast<IStreamingVoiceTaskSession *>(session->storage.get());
    if (streaming == nullptr) {
        return;
    }
    if (cb == nullptr) {
        streaming->set_stream_event_sink(nullptr);
        return;
    }
    EventSink * sink = &session->sink;
    streaming->set_stream_event_sink([sink](const StreamEvent & event) {
        if (sink->cb != nullptr) {
            const std::string json_str = json::stringify(dump_stream_event(event));
            sink->cb(sink->user_data, json_str.c_str(), event.is_final ? 1 : 0);
        }
    });
}

int audiocpp_session_start(audiocpp_session * session, const char * request_json) {
    if (session == nullptr || request_json == nullptr) {
        set_last_error("null session or request_json");
        return -1;
    }
    try {
        auto * streaming = dynamic_cast<IStreamingVoiceTaskSession *>(session->storage.get());
        if (streaming == nullptr) {
            set_last_error("session does not support streaming");
            return -1;
        }
        const json::Value root = json::parse(request_json);
        const auto request = parse_task_request(root);
        session->storage->prepare(engine::runtime::build_preparation_request(request));
        streaming->start_stream(request);
        return 0;
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return -1;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_session_start");
        return -1;
    }
}

int audiocpp_session_process_audio(audiocpp_session * session,
                                   const float * samples,
                                   size_t count,
                                   int sample_rate,
                                   int channels,
                                   int64_t start_sample,
                                   char ** out_event_json) {
    if (out_event_json != nullptr) {
        *out_event_json = nullptr;
    }
    if (session == nullptr || (count != 0 && samples == nullptr)) {
        set_last_error("null session or samples");
        return -1;
    }
    try {
        auto * streaming = dynamic_cast<IStreamingVoiceTaskSession *>(session->storage.get());
        if (streaming == nullptr) {
            set_last_error("session does not support streaming");
            return -1;
        }
        engine::runtime::AudioChunk chunk;
        chunk.sample_rate = sample_rate;
        chunk.channels = channels;
        chunk.start_sample = start_sample;
        chunk.samples.assign(samples, samples + count);
        const StreamEvent event = streaming->process_audio_chunk(chunk);
        if (out_event_json != nullptr) {
            *out_event_json = dup_string(json::stringify(dump_stream_event(event)));
        }
        return 0;
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return -1;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_session_process_audio");
        return -1;
    }
}

int audiocpp_session_finish(audiocpp_session * session, char ** out_json) {
    if (out_json != nullptr) {
        *out_json = nullptr;
    }
    if (session == nullptr) {
        set_last_error("null session");
        return -1;
    }
    try {
        auto * streaming = dynamic_cast<IStreamingVoiceTaskSession *>(session->storage.get());
        if (streaming == nullptr) {
            set_last_error("session does not support streaming");
            return -1;
        }
        const TaskResult result = streaming->finish_stream();
        if (out_json != nullptr) {
            *out_json = dup_string(json::stringify(dump_task_result(result)));
        }
        return 0;
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
        return -1;
    } catch (...) {
        set_last_error("unknown exception in audiocpp_session_finish");
        return -1;
    }
}

void audiocpp_session_reset(audiocpp_session * session) {
    if (session == nullptr) {
        return;
    }
    try {
        auto * streaming = dynamic_cast<IStreamingVoiceTaskSession *>(session->storage.get());
        if (streaming != nullptr) {
            streaming->reset();
        }
    } catch (const std::exception & ex) {
        set_last_error(ex.what());
    } catch (...) {
        set_last_error("unknown exception in audiocpp_session_reset");
    }
}