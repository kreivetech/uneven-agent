#[derive(Debug, Clone, Copy)]
pub struct ModelFile {
    pub url: &'static str,
    pub dest: &'static str,
    pub size: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct LocalModelEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub ram: &'static str,
    pub storage: &'static str,
    pub files: &'static [ModelFile],
}

pub static EMBEDDING_FILES: &[ModelFile] = &[
    ModelFile {
        url: "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/model.safetensors",
        dest: ".uneven/models/embeddings/model.safetensors",
        size: "~90 MB",
    },
    ModelFile {
        url: "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json",
        dest: ".uneven/models/embeddings/tokenizer.json",
        size: "~700 KB",
    },
    ModelFile {
        url: "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/config.json",
        dest: ".uneven/models/embeddings/config.json",
        size: "~1 KB",
    },
];

pub static LOCAL_MODEL_CATALOG: &[LocalModelEntry] = &[
    LocalModelEntry {
        id: "llama-3.2-1b-q4",
        label: "Llama 3.2 1B Q4",
        ram: "~1.5 GB",
        storage: "~700 MB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf",
                dest: ".uneven/models/llama-3.2-1b-q4.gguf",
                size: "~700 MB",
            },
            ModelFile {
                url: "https://huggingface.co/unsloth/Llama-3.2-1B-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/llama-3.2-1b-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "qwen-2.5-1.5b-q8",
        label: "Qwen 2.5 1.5B Q8",
        ram: "~3 GB",
        storage: "~1.6 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q8_0.gguf",
                dest: ".uneven/models/qwen-2.5-1.5b-q8.gguf",
                size: "~1.6 GB",
            },
            ModelFile {
                url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/qwen-2.5-1.5b-q8-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "deepseek-r1-1.5b-q8",
        label: "DeepSeek-R1 Distill 1.5B Q8",
        ram: "~3 GB",
        storage: "~1.6 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/DeepSeek-R1-Distill-Qwen-1.5B-GGUF/resolve/main/DeepSeek-R1-Distill-Qwen-1.5B-Q8_0.gguf",
                dest: ".uneven/models/deepseek-r1-1.5b-q8.gguf",
                size: "~1.6 GB",
            },
            ModelFile {
                url: "https://huggingface.co/deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B/resolve/main/tokenizer.json",
                dest: ".uneven/models/deepseek-r1-1.5b-q8-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "gemma-3-1b-q8",
        label: "Gemma 3 1B Q8",
        ram: "~3.5 GB",
        storage: "~1.1 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/gemma-3-1b-it-GGUF/resolve/main/gemma-3-1b-it-Q8_0.gguf",
                dest: ".uneven/models/gemma-3-1b-q8.gguf",
                size: "~1.1 GB",
            },
            ModelFile {
                url: "https://huggingface.co/google/gemma-3-1b-it/resolve/main/tokenizer.json",
                dest: ".uneven/models/gemma-3-1b-q8-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "smollm2-1.7b-q8",
        label: "SmolLM2 1.7B Q8",
        ram: "~3 GB",
        storage: "~1.8 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/SmolLM2-1.7B-Instruct-GGUF/resolve/main/SmolLM2-1.7B-Instruct-Q8_0.gguf",
                dest: ".uneven/models/smollm2-1.7b-q8.gguf",
                size: "~1.8 GB",
            },
            ModelFile {
                url: "https://huggingface.co/HuggingFaceTB/SmolLM2-1.7B-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/smollm2-1.7b-q8-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "phi-4-mini-q4",
        label: "Phi-4 Mini 3.8B Q4",
        ram: "~3.8 GB",
        storage: "~2.4 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/phi-4-mini-instruct-GGUF/resolve/main/phi-4-mini-instruct-Q4_K_M.gguf",
                dest: ".uneven/models/phi-4-mini-q4.gguf",
                size: "~2.4 GB",
            },
            ModelFile {
                url: "https://huggingface.co/microsoft/phi-4-mini-instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/phi-4-mini-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "qwen-2.5-3b-q4",
        label: "Qwen 2.5 3B Q4",
        ram: "~3.5 GB",
        storage: "~1.9 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf",
                dest: ".uneven/models/qwen-2.5-3b-q4.gguf",
                size: "~1.9 GB",
            },
            ModelFile {
                url: "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/qwen-2.5-3b-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "qwen-2.5-7b-q4",
        label: "Qwen 2.5 7B Instruct Q4",
        ram: "~8 GB",
        storage: "~4.7 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q4_K_M.gguf",
                dest: ".uneven/models/qwen-2.5-7b-q4.gguf",
                size: "~4.7 GB",
            },
            ModelFile {
                url: "https://huggingface.co/Qwen/Qwen2.5-7B-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/qwen-2.5-7b-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "deepseek-r1-8b-q4",
        label: "DeepSeek R1 Distill Qwen 8B Q4",
        ram: "~9 GB",
        storage: "~4.9 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/DeepSeek-R1-Distill-Qwen-8B-GGUF/resolve/main/DeepSeek-R1-Distill-Qwen-8B-Q4_K_M.gguf",
                dest: ".uneven/models/deepseek-r1-8b-q4.gguf",
                size: "~4.9 GB",
            },
            ModelFile {
                url: "https://huggingface.co/deepseek-ai/DeepSeek-R1-Distill-Qwen-8B/resolve/main/tokenizer.json",
                dest: ".uneven/models/deepseek-r1-8b-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "phi-4-14b-q4",
        label: "Phi-4 14B Instruct Q4",
        ram: "~16 GB",
        storage: "~9.1 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/phi-4-GGUF/resolve/main/phi-4-Q4_K_M.gguf",
                dest: ".uneven/models/phi-4-14b-q4.gguf",
                size: "~9.1 GB",
            },
            ModelFile {
                url: "https://huggingface.co/unsloth/phi-4/resolve/main/tokenizer.json",
                dest: ".uneven/models/phi-4-14b-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "llama-3.3-70b-q4",
        label: "Llama 3.3 70B Instruct Q4",
        ram: "~45 GB",
        storage: "~42 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/Llama-3.3-70B-Instruct-GGUF/resolve/main/Llama-3.3-70B-Instruct-Q4_K_M.gguf",
                dest: ".uneven/models/llama-3.3-70b-q4.gguf",
                size: "~42 GB",
            },
            ModelFile {
                url: "https://huggingface.co/unsloth/Llama-3.3-70B-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/llama-3.3-70b-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
];

pub static SUPPORTED_LOCAL_MODELS: &[LocalModelEntry] = &[
    LocalModelEntry {
        id: "smollm2-135m-q4",
        label: "SmolLM2 135M Q4 (Ultra-Light)",
        ram: "~200 MB",
        storage: "~90 MB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/SmolLM2-135M-Instruct-GGUF/resolve/main/SmolLM2-135M-Instruct-Q4_K_M.gguf",
                dest: ".uneven/models/smollm2-135m-q4.gguf",
                size: "~90 MB",
            },
            ModelFile {
                url: "https://huggingface.co/HuggingFaceTB/SmolLM2-135M-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/smollm2-135m-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "qwen-2.5-0.5b-q4",
        label: "Qwen 2.5 0.5B Q4 (Ultra-Light)",
        ram: "~500 MB",
        storage: "~390 MB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf",
                dest: ".uneven/models/qwen-2.5-0.5b-q4.gguf",
                size: "~390 MB",
            },
            ModelFile {
                url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/qwen-2.5-0.5b-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "llama-3.2-1b-q4",
        label: "Llama 3.2 1B Q4",
        ram: "~1.5 GB",
        storage: "~700 MB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf",
                dest: ".uneven/models/llama-3.2-1b-q4.gguf",
                size: "~700 MB",
            },
            ModelFile {
                url: "https://huggingface.co/unsloth/Llama-3.2-1B-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/llama-3.2-1b-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "qwen-2.5-1.5b-q8",
        label: "Qwen 2.5 1.5B Q8",
        ram: "~3 GB",
        storage: "~1.6 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q8_0.gguf",
                dest: ".uneven/models/qwen-2.5-1.5b-q8.gguf",
                size: "~1.6 GB",
            },
            ModelFile {
                url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/qwen-2.5-1.5b-q8-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "deepseek-r1-1.5b-q8",
        label: "DeepSeek-R1 Distill 1.5B Q8",
        ram: "~3 GB",
        storage: "~1.6 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/DeepSeek-R1-Distill-Qwen-1.5B-GGUF/resolve/main/DeepSeek-R1-Distill-Qwen-1.5B-Q8_0.gguf",
                dest: ".uneven/models/deepseek-r1-1.5b-q8.gguf",
                size: "~1.6 GB",
            },
            ModelFile {
                url: "https://huggingface.co/deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B/resolve/main/tokenizer.json",
                dest: ".uneven/models/deepseek-r1-1.5b-q8-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "gemma-3-1b-q8",
        label: "Gemma 3 1B Q8",
        ram: "~3.5 GB",
        storage: "~1.1 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/gemma-3-1b-it-GGUF/resolve/main/gemma-3-1b-it-Q8_0.gguf",
                dest: ".uneven/models/gemma-3-1b-q8.gguf",
                size: "~1.1 GB",
            },
            ModelFile {
                url: "https://huggingface.co/google/gemma-3-1b-it/resolve/main/tokenizer.json",
                dest: ".uneven/models/gemma-3-1b-q8-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "smollm2-1.7b-q8",
        label: "SmolLM2 1.7B Q8",
        ram: "~3 GB",
        storage: "~1.8 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/SmolLM2-1.7B-Instruct-GGUF/resolve/main/SmolLM2-1.7B-Instruct-Q8_0.gguf",
                dest: ".uneven/models/smollm2-1.7b-q8.gguf",
                size: "~1.8 GB",
            },
            ModelFile {
                url: "https://huggingface.co/HuggingFaceTB/SmolLM2-1.7B-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/smollm2-1.7b-q8-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "phi-4-mini-q4",
        label: "Phi-4 Mini 3.8B Q4",
        ram: "~3.8 GB",
        storage: "~2.4 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/phi-4-mini-instruct-GGUF/resolve/main/phi-4-mini-instruct-Q4_K_M.gguf",
                dest: ".uneven/models/phi-4-mini-q4.gguf",
                size: "~2.4 GB",
            },
            ModelFile {
                url: "https://huggingface.co/microsoft/phi-4-mini-instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/phi-4-mini-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "qwen-2.5-3b-q4",
        label: "Qwen 2.5 3B Q4",
        ram: "~3.5 GB",
        storage: "~1.9 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf",
                dest: ".uneven/models/qwen-2.5-3b-q4.gguf",
                size: "~1.9 GB",
            },
            ModelFile {
                url: "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/qwen-2.5-3b-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "qwen-2.5-7b-q4",
        label: "Qwen 2.5 7B Instruct Q4",
        ram: "~8 GB",
        storage: "~4.7 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q4_K_M.gguf",
                dest: ".uneven/models/qwen-2.5-7b-q4.gguf",
                size: "~4.7 GB",
            },
            ModelFile {
                url: "https://huggingface.co/Qwen/Qwen2.5-7B-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/qwen-2.5-7b-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "deepseek-r1-8b-q4",
        label: "DeepSeek R1 Distill Qwen 8B Q4",
        ram: "~9 GB",
        storage: "~4.9 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/DeepSeek-R1-Distill-Qwen-8B-GGUF/resolve/main/DeepSeek-R1-Distill-Qwen-8B-Q4_K_M.gguf",
                dest: ".uneven/models/deepseek-r1-8b-q4.gguf",
                size: "~4.9 GB",
            },
            ModelFile {
                url: "https://huggingface.co/deepseek-ai/DeepSeek-R1-Distill-Qwen-8B/resolve/main/tokenizer.json",
                dest: ".uneven/models/deepseek-r1-8b-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "phi-4-14b-q4",
        label: "Phi-4 14B Instruct Q4",
        ram: "~16 GB",
        storage: "~9.1 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/phi-4-GGUF/resolve/main/phi-4-Q4_K_M.gguf",
                dest: ".uneven/models/phi-4-14b-q4.gguf",
                size: "~9.1 GB",
            },
            ModelFile {
                url: "https://huggingface.co/unsloth/phi-4/resolve/main/tokenizer.json",
                dest: ".uneven/models/phi-4-14b-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
    LocalModelEntry {
        id: "llama-3.3-70b-q4",
        label: "Llama 3.3 70B Instruct Q4",
        ram: "~45 GB",
        storage: "~42 GB",
        files: &[
            ModelFile {
                url: "https://huggingface.co/bartowski/Llama-3.3-70B-Instruct-GGUF/resolve/main/Llama-3.3-70B-Instruct-Q4_K_M.gguf",
                dest: ".uneven/models/llama-3.3-70b-q4.gguf",
                size: "~42 GB",
            },
            ModelFile {
                url: "https://huggingface.co/unsloth/Llama-3.3-70B-Instruct/resolve/main/tokenizer.json",
                dest: ".uneven/models/llama-3.3-70b-q4-tokenizer.json",
                size: "~1.7 MB",
            },
        ],
    },
];
