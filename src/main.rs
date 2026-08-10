//! sample-mario: マリオ風2Dプラットフォーマーのショートゲームプロトタイプ
//! (Phase 0)。`open-directx`のDXBC->SPIR-V翻訳済みスプライトパイプライン
//! (`sample-harrier`/`sample-maid`と同じRenderer)を再利用。
//!
//! **正直な開示(現在のスコープ)**: 重力+ジャンプ+地面/床の当たり判定+
//! ゴール到達判定という最小限のプラットフォーマー要素のみ。複数ステージ・
//! スクロールする広いマップは未実装(次回以降のロードマップ)。
//!
//! 内容: 640x480ウィンドウ。地面(緑)+浮いた足場1枚(茶)+自機(赤)+
//! ゴール旗(黄)+ボス(甲羅の魔竜王、独自デザイン)。矢印キー/AD左右
//! 移動、Space/Upでジャンプ、Fキーで火の玉を発射してボスを攻撃できる
//! (ユーザー指示、2026-08-10「マリオ風キャラが炎で(ボスを)攻撃出来ると
//! 良い」への対応)。ゴールに到達すると"CLEAR!"をログ出力し自機を開始
//! 位置へ戻す(簡易ループ)。BGMは既存楽曲の再現ではなく、8bit風の
//! オリジナル曲(`audio.rs`)をループ再生する。
//!
//! **著作権上の注意(正直な開示)**: 「クッパ」はNintendoの登録商標・
//! キャラクターであり、その名称・意匠を再現することはできないため、
//! ここでは"甲羅を持つ魔竜王"という独自デザインのボスとして実装している
//! (見た目は単色スプライトのプレースホルダー、Phase 0)。BGMも実際の
//! マリオシリーズ楽曲の転用ではなく、8bit・軽快なテンポという曲想のみを
//! 参考にした新規オリジナル曲。
//!
//! 追加ギミック(ユーザー指示、2026-08-10「マリオはキャラクターも作って
//! 構造物と当たり判定を作って亀を敵にして」「全画面表示、操作説明を下側に
//! 英日で」への対応):
//! - 亀の敵(緑)が地面を徘徊し、上から踏むと撃破・側面に当たると自機が
//!   開始位置へ戻される(接触後は`TURTLE_HIT_COOLDOWN_FRAMES`フレームの
//!   無敵時間)。
//! - 全画面表示(ボーダーレスフルスクリーン)。
//! - 画面下にHUDバナー(英文、自作5x7ビットマップフォント、`font.rs`)で
//!   操作説明を常時表示。日本語の操作説明はコンソール出力で対応する
//!   (日本語グリフは用意していない、正直な開示)。
//!
//! 実行方法: `cargo run --release`

mod audio;
mod font;

use std::ffi::CString;

use ash::{vk, Entry};
use directx_graphics_vulkan::{Rgba8, TextureRgba8};
use directx_shader_translate::spirv_gen::{translate_sprite_pixel_shader, translate_sprite_vertex_shader};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::event::{ElementState, Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

const GRAVITY: f32 = -0.0022;
const JUMP_VELOCITY: f32 = 0.045;
const MOVE_SPEED: f32 = 0.018;
const PLAYER_HALF: f32 = 0.06;
const GROUND_TOP: f32 = 0.8;
const PLATFORM_Y_TOP: f32 = 0.25;
const PLATFORM_Y_BOTTOM: f32 = 0.32;
const PLATFORM_X0: f32 = 0.1;
const PLATFORM_X1: f32 = 0.5;
const GOAL_X: f32 = 0.85;

const BOSS_X: f32 = 0.65;
const BOSS_HALF: f32 = 0.09;
const BOSS_MAX_HP: f32 = 1.0;
const MAX_FIREBALLS: usize = 4;
const FIREBALL_SPEED: f32 = 0.03;
const FIREBALL_COOLDOWN_FRAMES: u32 = 18;
const FIREBALL_DAMAGE: f32 = 0.2;

#[derive(Clone, Copy)]
struct Fireball {
    active: bool,
    x: f32,
    y: f32,
}

impl Fireball {
    const INACTIVE: Fireball = Fireball { active: false, x: 0.0, y: 0.0 };
}

const TURTLE_COUNT: usize = 2;
const TURTLE_HALF: f32 = 0.05;
const TURTLE_SPEED: f32 = 0.006;
const TURTLE_HIT_COOLDOWN_FRAMES: u32 = 60;

#[derive(Clone, Copy)]
struct Turtle {
    alive: bool,
    x: f32,
    range_x0: f32,
    range_x1: f32,
    dir: f32,
}

impl Turtle {
    fn new(range_x0: f32, range_x1: f32) -> Self {
        Turtle { alive: true, x: (range_x0 + range_x1) * 0.5, range_x0, range_x1, dir: 1.0 }
    }

    fn y(&self) -> f32 {
        GROUND_TOP - TURTLE_HALF
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 3],
    uv: [f32; 2],
}

fn quad_vertices(dest_ndc: [f32; 4]) -> [Vertex; 6] {
    let [x0, y0, x1, y1] = dest_ndc;
    let tl = Vertex { pos: [x0, y0, 0.0], uv: [0.0, 0.0] };
    let tr = Vertex { pos: [x1, y0, 0.0], uv: [1.0, 0.0] };
    let br = Vertex { pos: [x1, y1, 0.0], uv: [1.0, 1.0] };
    let bl = Vertex { pos: [x0, y1, 0.0], uv: [0.0, 1.0] };
    [tl, tr, br, tl, br, bl]
}

struct GameState {
    player_x: f32,
    player_y: f32,
    vel_y: f32,
    on_ground: bool,
    left_held: bool,
    right_held: bool,
    jump_held: bool,
    clears: u32,
    fire_held: bool,
    fire_cooldown: u32,
    fireballs: [Fireball; MAX_FIREBALLS],
    boss_hp: f32,
    boss_defeats: u32,
    turtles: [Turtle; TURTLE_COUNT],
    turtle_hit_cooldown: u32,
}

impl GameState {
    fn new() -> Self {
        GameState {
            player_x: -0.85,
            player_y: GROUND_TOP - PLAYER_HALF,
            vel_y: 0.0,
            on_ground: true,
            left_held: false,
            right_held: false,
            jump_held: false,
            clears: 0,
            fire_held: false,
            fire_cooldown: 0,
            fireballs: [Fireball::INACTIVE; MAX_FIREBALLS],
            turtles: [Turtle::new(-0.65, -0.2), Turtle::new(0.55, 0.75)],
            turtle_hit_cooldown: 0,
            boss_hp: BOSS_MAX_HP,
            boss_defeats: 0,
        }
    }

    fn update(&mut self) {
        if self.left_held {
            self.player_x -= MOVE_SPEED;
        }
        if self.right_held {
            self.player_x += MOVE_SPEED;
        }
        self.player_x = self.player_x.clamp(-1.0 + PLAYER_HALF, 1.0 - PLAYER_HALF);

        if self.jump_held && self.on_ground {
            self.vel_y = JUMP_VELOCITY;
            self.on_ground = false;
        }

        self.vel_y += GRAVITY;
        self.player_y -= self.vel_y;

        // 地面判定。
        if self.player_y + PLAYER_HALF >= GROUND_TOP {
            self.player_y = GROUND_TOP - PLAYER_HALF;
            self.vel_y = 0.0;
            self.on_ground = true;
        }

        // 足場判定(上から乗るときのみ、簡易AABB)。
        let on_platform_x = self.player_x + PLAYER_HALF > PLATFORM_X0 && self.player_x - PLAYER_HALF < PLATFORM_X1;
        if on_platform_x && self.vel_y < 0.0 {
            let feet_y = self.player_y + PLAYER_HALF;
            if feet_y >= PLATFORM_Y_TOP && feet_y - (-self.vel_y) <= PLATFORM_Y_TOP {
                self.player_y = PLATFORM_Y_TOP - PLAYER_HALF;
                self.vel_y = 0.0;
                self.on_ground = true;
            }
        }

        self.jump_held = false;

        // 亀の徘徊(左右往復)+自機との当たり判定(上から踏むと撃破、
        // 側面/下から当たると自機が開始位置へ戻る、被弾直後は無敵時間)。
        self.turtle_hit_cooldown = self.turtle_hit_cooldown.saturating_sub(1);
        let player_falling = self.vel_y < 0.0;
        let mut player_hurt = false;
        for turtle in self.turtles.iter_mut() {
            if !turtle.alive {
                continue;
            }
            turtle.x += TURTLE_SPEED * turtle.dir;
            if turtle.x <= turtle.range_x0 {
                turtle.x = turtle.range_x0;
                turtle.dir = 1.0;
            } else if turtle.x >= turtle.range_x1 {
                turtle.x = turtle.range_x1;
                turtle.dir = -1.0;
            }

            let dx = (self.player_x - turtle.x).abs();
            let overlap_x = dx < PLAYER_HALF + TURTLE_HALF;
            let player_feet = self.player_y + PLAYER_HALF;
            let turtle_top = turtle.y() - TURTLE_HALF;
            let turtle_bottom = turtle.y() + TURTLE_HALF;
            let overlap_y = player_feet > turtle_top && self.player_y - PLAYER_HALF < turtle_bottom;
            if overlap_x && overlap_y {
                if player_falling && player_feet - turtle_top < TURTLE_HALF {
                    // 上から踏んで撃破、軽くバウンドさせる。
                    turtle.alive = false;
                    self.vel_y = JUMP_VELOCITY * 0.6;
                    println!("亀を撃破!");
                } else if self.turtle_hit_cooldown == 0 {
                    player_hurt = true;
                }
            }
        }
        if player_hurt {
            self.turtle_hit_cooldown = TURTLE_HIT_COOLDOWN_FRAMES;
            println!("亀に接触してダメージ! 開始位置へ戻ります");
            self.player_x = -0.85;
            self.player_y = GROUND_TOP - PLAYER_HALF;
            self.vel_y = 0.0;
            self.on_ground = true;
        }

        // 火の玉の発射(Fキー)。クールダウン中・空きスロットが無ければ無視。
        if self.fire_held && self.fire_cooldown == 0 {
            if let Some(slot) = self.fireballs.iter_mut().find(|f| !f.active) {
                *slot = Fireball { active: true, x: self.player_x + PLAYER_HALF, y: self.player_y };
                self.fire_cooldown = FIREBALL_COOLDOWN_FRAMES;
            }
        }
        self.fire_cooldown = self.fire_cooldown.saturating_sub(1);

        // 火の玉の飛行とボスへの命中判定。
        let boss_y = GROUND_TOP - BOSS_HALF;
        for fireball in self.fireballs.iter_mut() {
            if !fireball.active {
                continue;
            }
            fireball.x += FIREBALL_SPEED;
            if fireball.x > 1.0 {
                fireball.active = false;
                continue;
            }
            let dx = (fireball.x - BOSS_X).abs();
            let dy = (fireball.y - boss_y).abs();
            if self.boss_hp > 0.0 && dx < BOSS_HALF && dy < BOSS_HALF {
                fireball.active = false;
                self.boss_hp = (self.boss_hp - FIREBALL_DAMAGE).max(0.0);
                if self.boss_hp <= 0.0 {
                    self.boss_defeats += 1;
                    println!("ボス撃破! (total={})", self.boss_defeats);
                } else {
                    println!("ボスに火の玉が命中! (残りHP: {:.0}%)", self.boss_hp * 100.0);
                }
            }
        }

        // ボスを倒すとゴールできる想定(倒す前はゴールできない、という
        // Phase 0の簡易ルール)。撃破後は次周のためHPを回復させる。
        if self.boss_hp <= 0.0 {
            // ゴール到達判定。
            if (self.player_x - GOAL_X).abs() < PLAYER_HALF + 0.05 && self.player_y < GROUND_TOP - 0.01 {
                self.clears += 1;
                println!("CLEAR! (total={})", self.clears);
                self.player_x = -0.85;
                self.player_y = GROUND_TOP - PLAYER_HALF;
                self.vel_y = 0.0;
                self.on_ground = true;
                self.boss_hp = BOSS_MAX_HP;
            }
        }
    }

    fn player_dest_ndc(&self) -> [f32; 4] {
        [self.player_x - PLAYER_HALF, self.player_y - PLAYER_HALF, self.player_x + PLAYER_HALF, self.player_y + PLAYER_HALF]
    }
    fn ground_dest_ndc(&self) -> [f32; 4] {
        [-1.0, GROUND_TOP, 1.0, 1.0]
    }
    fn platform_dest_ndc(&self) -> [f32; 4] {
        [PLATFORM_X0, PLATFORM_Y_TOP, PLATFORM_X1, PLATFORM_Y_BOTTOM]
    }
    fn goal_dest_ndc(&self) -> [f32; 4] {
        [GOAL_X - 0.03, GROUND_TOP - 0.35, GOAL_X + 0.03, GROUND_TOP]
    }
    fn boss_dest_ndc(&self) -> [f32; 4] {
        if self.boss_hp <= 0.0 {
            // 撃破後は次に倒すまで画面外へ(HP回復後は再表示)。
            return [-2.0, -2.0, -1.9, -1.9];
        }
        let y = GROUND_TOP - BOSS_HALF;
        [BOSS_X - BOSS_HALF, y - BOSS_HALF, BOSS_X + BOSS_HALF, y + BOSS_HALF]
    }
    fn boss_hp_bar_ndc(&self) -> [f32; 4] {
        let w = 0.3 * (self.boss_hp / BOSS_MAX_HP);
        [-0.15 - w, -0.95, -0.15 + w, -0.9]
    }
    fn fireball_dest_ndc(&self, i: usize) -> [f32; 4] {
        let f = &self.fireballs[i];
        if !f.active {
            return [-2.0, -2.0, -1.9, -1.9];
        }
        [f.x - 0.025, f.y - 0.025, f.x + 0.025, f.y + 0.025]
    }

    fn turtle_dest_ndc(&self, i: usize) -> [f32; 4] {
        let t = &self.turtles[i];
        if !t.alive {
            return [-2.0, -2.0, -1.9, -1.9];
        }
        let y = t.y();
        [t.x - TURTLE_HALF, y - TURTLE_HALF, t.x + TURTLE_HALF, y + TURTLE_HALF]
    }
}

fn main() {
    println!("=== sample-mario 操作方法 ===");
    println!("矢印キー/AD: 左右移動   Space/Up: ジャンプ   F: 火の玉発射   Esc または ウィンドウのXボタン: 終了");

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("sample-mario: 2D platformer prototype (Phase 0)")
        .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
        .build(&event_loop)
        .expect("create window");

    // BGM再生ハンドルはイベントループが動いている間ずっと保持する
    // (dropすると再生が止まるため、ここで`main`のスコープに留める。
    // `event_loop.run`は戻らないため、事実上プロセス終了までスコープに残る)。
    let _bgm = audio::play_bgm_loop();

    let mut renderer = unsafe { Renderer::new(&window) }.expect("create Vulkan renderer");
    let mut state = GameState::new();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent { event: WindowEvent::KeyboardInput { input, .. }, .. } => {
                let pressed = input.state == ElementState::Pressed;
                match input.virtual_keycode {
                    Some(VirtualKeyCode::Left) | Some(VirtualKeyCode::A) => state.left_held = pressed,
                    Some(VirtualKeyCode::Right) | Some(VirtualKeyCode::D) => state.right_held = pressed,
                    Some(VirtualKeyCode::Space) | Some(VirtualKeyCode::Up) if pressed => state.jump_held = true,
                    Some(VirtualKeyCode::F) => state.fire_held = pressed,
                    Some(VirtualKeyCode::Escape) => *control_flow = ControlFlow::Exit,
                    _ => {}
                }
            }
            Event::MainEventsCleared => {
                state.update();
                unsafe { renderer.draw_frame(&state) };
            }
            _ => {}
        }
    });
}

/// 実ウィンドウ向けVulkanレンダラー。`sample-harrier`/`sample-maid`/
/// `open-directx`の`directx-graphics-window`と同一パターン
/// (固定サイズスワップチェーン、単一コマンドバッファ+フェンス待機)。
struct Renderer {
    _entry: Entry,
    instance: ash::Instance,
    surface_loader: ash::extensions::khr::Surface,
    surface: vk::SurfaceKHR,
    device: ash::Device,
    queue: vk::Queue,
    swapchain_loader: ash::extensions::khr::Swapchain,
    swapchain: vk::SwapchainKHR,
    _swapchain_format: vk::Format,
    swapchain_extent: vk::Extent2D,
    swapchain_image_views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    ground_descriptor_set: vk::DescriptorSet,
    platform_descriptor_set: vk::DescriptorSet,
    player_descriptor_set: vk::DescriptorSet,
    goal_descriptor_set: vk::DescriptorSet,
    boss_descriptor_set: vk::DescriptorSet,
    boss_hp_descriptor_set: vk::DescriptorSet,
    fireball_descriptor_set: vk::DescriptorSet,
    turtle_descriptor_set: vk::DescriptorSet,
    hud_descriptor_set: vk::DescriptorSet,
    vertex_buffer: vk::Buffer,
    vertex_buffer_memory: vk::DeviceMemory,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    in_flight: vk::Fence,
    _ground_texture_image: vk::Image,
    _platform_texture_image: vk::Image,
    _player_texture_image: vk::Image,
    _goal_texture_image: vk::Image,
    _boss_texture_image: vk::Image,
    _boss_hp_texture_image: vk::Image,
    _fireball_texture_image: vk::Image,
    _turtle_texture_image: vk::Image,
    _hud_texture_image: vk::Image,
    sampler: vk::Sampler,
}

/// 固定スプライト(地面・足場・自機・ゴール旗・ボス・ボスHPバー)
/// +火の玉`MAX_FIREBALLS`発+亀`TURTLE_COUNT`匹+HUDバナー1枚を
/// 毎フレーム描画する。
const FIXED_SPRITE_COUNT: usize = 6;
const FIREBALL_SPRITE_OFFSET: usize = FIXED_SPRITE_COUNT;
const TURTLE_SPRITE_OFFSET: usize = FIREBALL_SPRITE_OFFSET + MAX_FIREBALLS;
const HUD_SPRITE_INDEX: usize = TURTLE_SPRITE_OFFSET + TURTLE_COUNT;
const DRAWN_SPRITE_COUNT: usize = HUD_SPRITE_INDEX + 1;
/// 一意のテクスチャ数(固定6種+火の玉+亀+HUDバナー)。
const TEXTURE_COUNT: usize = FIXED_SPRITE_COUNT + 3;
const VERTS_PER_SPRITE: usize = 6;
const HUD_TEXT: &str = "ARROWS/WASD MOVE  SPACE/UP JUMP  F FIRE  ESC/X CLOSE";
const HUD_HEIGHT_NDC: f32 = 0.08;

impl Renderer {
    unsafe fn new(window: &winit::window::Window) -> Result<Self, String> {
        let entry = Entry::load().map_err(|e| e.to_string())?;

        let app_name = CString::new("sample-mario").unwrap();
        let app_info = vk::ApplicationInfo::builder().application_name(&app_name).api_version(vk::API_VERSION_1_1);
        let display_handle = window.raw_display_handle();
        let window_handle = window.raw_window_handle();
        let required_extensions =
            ash_window::enumerate_required_extensions(display_handle).map_err(|e| e.to_string())?;
        let instance_info = vk::InstanceCreateInfo::builder().application_info(&app_info).enabled_extension_names(required_extensions);
        let instance = entry.create_instance(&instance_info, None).map_err(|e| e.to_string())?;

        let surface = ash_window::create_surface(&entry, &instance, display_handle, window_handle, None).map_err(|e| e.to_string())?;
        let surface_loader = ash::extensions::khr::Surface::new(&entry, &instance);

        let physical_devices = instance.enumerate_physical_devices().map_err(|e| e.to_string())?;
        let mut selected: Option<(vk::PhysicalDevice, u32)> = None;
        for &pd in &physical_devices {
            let families = instance.get_physical_device_queue_family_properties(pd);
            for (idx, f) in families.iter().enumerate() {
                let supports_present = surface_loader.get_physical_device_surface_support(pd, idx as u32, surface).unwrap_or(false);
                if f.queue_flags.contains(vk::QueueFlags::GRAPHICS) && supports_present {
                    selected = Some((pd, idx as u32));
                    break;
                }
            }
            if selected.is_some() {
                break;
            }
        }
        let (physical_device, queue_family_index) = selected.ok_or("no graphics+present capable device found")?;

        let priorities = [1.0f32];
        let queue_info = [vk::DeviceQueueCreateInfo::builder().queue_family_index(queue_family_index).queue_priorities(&priorities).build()];
        let device_extensions = [ash::extensions::khr::Swapchain::name().as_ptr()];
        let device_info = vk::DeviceCreateInfo::builder().queue_create_infos(&queue_info).enabled_extension_names(&device_extensions);
        let device = instance.create_device(physical_device, &device_info, None).map_err(|e| e.to_string())?;
        let queue = device.get_device_queue(queue_family_index, 0);
        let memory_properties = instance.get_physical_device_memory_properties(physical_device);

        let swapchain_loader = ash::extensions::khr::Swapchain::new(&instance, &device);
        let surface_caps = surface_loader.get_physical_device_surface_capabilities(physical_device, surface).map_err(|e| e.to_string())?;
        let surface_formats = surface_loader.get_physical_device_surface_formats(physical_device, surface).map_err(|e| e.to_string())?;
        let surface_format = surface_formats
            .iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_UNORM)
            .copied()
            .unwrap_or(surface_formats[0]);
        let present_mode = vk::PresentModeKHR::FIFO;
        let window_size = window.inner_size();
        let extent = vk::Extent2D { width: window_size.width.max(1), height: window_size.height.max(1) };
        let image_count = (surface_caps.min_image_count + 1).min(if surface_caps.max_image_count > 0 { surface_caps.max_image_count } else { u32::MAX });

        let swapchain_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(surface_caps.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true);
        let swapchain = swapchain_loader.create_swapchain(&swapchain_info, None).map_err(|e| e.to_string())?;
        let swapchain_images = swapchain_loader.get_swapchain_images(swapchain).map_err(|e| e.to_string())?;

        let swapchain_image_views: Vec<vk::ImageView> = swapchain_images
            .iter()
            .map(|&img| {
                let info = vk::ImageViewCreateInfo::builder()
                    .image(img)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(surface_format.format)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });
                device.create_image_view(&info, None).expect("create swapchain image view")
            })
            .collect();

        let attachment = vk::AttachmentDescription::builder()
            .format(surface_format.format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .build();
        let color_ref = vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL };
        let color_refs = [color_ref];
        let subpass = vk::SubpassDescription::builder().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&color_refs).build();
        let attachments = [attachment];
        let subpasses = [subpass];
        let dependency = vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .build();
        let dependencies = [dependency];
        let render_pass_info = vk::RenderPassCreateInfo::builder().attachments(&attachments).subpasses(&subpasses).dependencies(&dependencies);
        let render_pass = device.create_render_pass(&render_pass_info, None).map_err(|e| e.to_string())?;

        let framebuffers: Vec<vk::Framebuffer> = swapchain_image_views
            .iter()
            .map(|&view| {
                let attachments_fb = [view];
                let info = vk::FramebufferCreateInfo::builder()
                    .render_pass(render_pass)
                    .attachments(&attachments_fb)
                    .width(extent.width)
                    .height(extent.height)
                    .layers(1);
                device.create_framebuffer(&info, None).expect("create framebuffer")
            })
            .collect();

        let (ground_image, ground_view) = create_solid_texture(&device, &memory_properties, queue, queue_family_index, Rgba8 { r: 30, g: 140, b: 40, a: 255 })?;
        let (platform_image, platform_view) = create_solid_texture(&device, &memory_properties, queue, queue_family_index, Rgba8 { r: 150, g: 100, b: 40, a: 255 })?;
        let (player_image, player_view) = create_solid_texture(&device, &memory_properties, queue, queue_family_index, Rgba8 { r: 220, g: 30, b: 30, a: 255 })?;
        let (goal_image, goal_view) = create_solid_texture(&device, &memory_properties, queue, queue_family_index, Rgba8 { r: 240, g: 220, b: 30, a: 255 })?;
        // ボス(甲羅の魔竜王、独自デザイン)=深緑、HPバー=赤、火の玉=橙。
        let (boss_image, boss_view) = create_solid_texture(&device, &memory_properties, queue, queue_family_index, Rgba8 { r: 40, g: 110, b: 60, a: 255 })?;
        let (boss_hp_image, boss_hp_view) = create_solid_texture(&device, &memory_properties, queue, queue_family_index, Rgba8 { r: 220, g: 50, b: 50, a: 255 })?;
        let (fireball_image, fireball_view) = create_solid_texture(&device, &memory_properties, queue, queue_family_index, Rgba8 { r: 255, g: 130, b: 20, a: 255 })?;
        let (turtle_image, turtle_view) = create_solid_texture(&device, &memory_properties, queue, queue_family_index, Rgba8 { r: 60, g: 170, b: 70, a: 255 })?;
        let hud_texture = font::render_text_banner(HUD_TEXT, 3, Rgba8 { r: 255, g: 255, b: 255, a: 255 }, Rgba8 { r: 10, g: 10, b: 15, a: 255 });
        let (hud_image, hud_view) = create_texture_from_rgba(&device, &memory_properties, queue, queue_family_index, &hud_texture)?;

        let sampler_info = vk::SamplerCreateInfo::builder().mag_filter(vk::Filter::NEAREST).min_filter(vk::Filter::NEAREST);
        let sampler = device.create_sampler(&sampler_info, None).map_err(|e| e.to_string())?;

        let dsl_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .build();
        let dsl_bindings = [dsl_binding];
        let dsl_info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(&dsl_bindings);
        let descriptor_set_layout = device.create_descriptor_set_layout(&dsl_info, None).map_err(|e| e.to_string())?;

        let pool_size = vk::DescriptorPoolSize { ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER, descriptor_count: TEXTURE_COUNT as u32 };
        let pool_sizes = [pool_size];
        let descriptor_pool_info = vk::DescriptorPoolCreateInfo::builder().pool_sizes(&pool_sizes).max_sets(TEXTURE_COUNT as u32);
        let descriptor_pool = device.create_descriptor_pool(&descriptor_pool_info, None).map_err(|e| e.to_string())?;

        let set_layouts = vec![descriptor_set_layout; TEXTURE_COUNT];
        let ds_alloc_info = vk::DescriptorSetAllocateInfo::builder().descriptor_pool(descriptor_pool).set_layouts(&set_layouts);
        let sets = device.allocate_descriptor_sets(&ds_alloc_info).map_err(|e| e.to_string())?;
        let (
            ground_descriptor_set,
            platform_descriptor_set,
            player_descriptor_set,
            goal_descriptor_set,
            boss_descriptor_set,
            boss_hp_descriptor_set,
            fireball_descriptor_set,
            turtle_descriptor_set,
            hud_descriptor_set,
        ) = (sets[0], sets[1], sets[2], sets[3], sets[4], sets[5], sets[6], sets[7], sets[8]);

        for (set, view) in [
            (ground_descriptor_set, ground_view),
            (platform_descriptor_set, platform_view),
            (player_descriptor_set, player_view),
            (goal_descriptor_set, goal_view),
            (boss_descriptor_set, boss_view),
            (boss_hp_descriptor_set, boss_hp_view),
            (fireball_descriptor_set, fireball_view),
            (turtle_descriptor_set, turtle_view),
            (hud_descriptor_set, hud_view),
        ] {
            let image_info = vk::DescriptorImageInfo::builder()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(view)
                .sampler(sampler)
                .build();
            let image_infos = [image_info];
            let write = vk::WriteDescriptorSet::builder()
                .dst_set(set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&image_infos)
                .build();
            device.update_descriptor_sets(&[write], &[]);
        }

        let vbuf_size = ((DRAWN_SPRITE_COUNT * VERTS_PER_SPRITE) * std::mem::size_of::<Vertex>()) as vk::DeviceSize;
        let (vertex_buffer, vertex_buffer_memory) =
            create_host_visible_buffer(&device, &memory_properties, vbuf_size, vk::BufferUsageFlags::VERTEX_BUFFER)?;

        let vs = translate_sprite_vertex_shader(include_bytes!("../../open-directx/crates/directx-shader-translate/shaders/sprite_vs.dxbc")).map_err(|e| e.to_string())?;
        let ps = translate_sprite_pixel_shader(include_bytes!("../../open-directx/crates/directx-shader-translate/shaders/sprite_ps.dxbc")).map_err(|e| e.to_string())?;

        let vs_module_info = vk::ShaderModuleCreateInfo::builder().code(&vs.spirv_words);
        let vs_module = device.create_shader_module(&vs_module_info, None).map_err(|e| e.to_string())?;
        let ps_module_info = vk::ShaderModuleCreateInfo::builder().code(&ps.spirv_words);
        let ps_module = device.create_shader_module(&ps_module_info, None).map_err(|e| e.to_string())?;

        let entry_name = CString::new("main").unwrap();
        let stages = [
            vk::PipelineShaderStageCreateInfo::builder().stage(vk::ShaderStageFlags::VERTEX).module(vs_module).name(&entry_name).build(),
            vk::PipelineShaderStageCreateInfo::builder().stage(vk::ShaderStageFlags::FRAGMENT).module(ps_module).name(&entry_name).build(),
        ];

        let binding_desc = vk::VertexInputBindingDescription { binding: 0, stride: std::mem::size_of::<Vertex>() as u32, input_rate: vk::VertexInputRate::VERTEX };
        let attr_descs = [
            vk::VertexInputAttributeDescription { location: 0, binding: 0, format: vk::Format::R32G32B32_SFLOAT, offset: 0 },
            vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32_SFLOAT, offset: std::mem::size_of::<[f32; 3]>() as u32 },
        ];
        let binding_descs = [binding_desc];
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::builder().vertex_binding_descriptions(&binding_descs).vertex_attribute_descriptions(&attr_descs);
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::builder().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport = vk::Viewport { x: 0.0, y: 0.0, width: extent.width as f32, height: extent.height as f32, min_depth: 0.0, max_depth: 1.0 };
        let scissor = vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent };
        let viewports = [viewport];
        let scissors = [scissor];
        let viewport_state = vk::PipelineViewportStateCreateInfo::builder().viewports(&viewports).scissors(&scissors);
        let rasterization = vk::PipelineRasterizationStateCreateInfo::builder()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let multisample = vk::PipelineMultisampleStateCreateInfo::builder().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let color_blend_attachment = vk::PipelineColorBlendAttachmentState { blend_enable: vk::FALSE, color_write_mask: vk::ColorComponentFlags::RGBA, ..Default::default() };
        let color_blend_attachments = [color_blend_attachment];
        let color_blend = vk::PipelineColorBlendStateCreateInfo::builder().attachments(&color_blend_attachments);
        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder().set_layouts(&set_layouts[..1]);
        let pipeline_layout = device.create_pipeline_layout(&pipeline_layout_info, None).map_err(|e| e.to_string())?;
        let pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization)
            .multisample_state(&multisample)
            .color_blend_state(&color_blend)
            .layout(pipeline_layout)
            .render_pass(render_pass)
            .subpass(0)
            .build();
        let pipeline = device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
            .map_err(|(_, e)| e.to_string())?[0];
        device.destroy_shader_module(vs_module, None);
        device.destroy_shader_module(ps_module, None);

        let pool_info = vk::CommandPoolCreateInfo::builder().queue_family_index(queue_family_index).flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = device.create_command_pool(&pool_info, None).map_err(|e| e.to_string())?;
        let cmd_alloc_info = vk::CommandBufferAllocateInfo::builder().command_pool(command_pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1);
        let command_buffer = device.allocate_command_buffers(&cmd_alloc_info).map_err(|e| e.to_string())?[0];

        let sem_info = vk::SemaphoreCreateInfo::builder();
        let image_available = device.create_semaphore(&sem_info, None).map_err(|e| e.to_string())?;
        let render_finished = device.create_semaphore(&sem_info, None).map_err(|e| e.to_string())?;
        let fence_info = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);
        let in_flight = device.create_fence(&fence_info, None).map_err(|e| e.to_string())?;

        println!(
            "sample-mario: 実ウィンドウ+実スワップチェーンを作成しました({}x{}, format_raw={}, images={})",
            extent.width,
            extent.height,
            surface_format.format.as_raw(),
            swapchain_images.len()
        );

        Ok(Renderer {
            _entry: entry,
            instance,
            surface_loader,
            surface,
            device,
            queue,
            swapchain_loader,
            swapchain,
            _swapchain_format: surface_format.format,
            swapchain_extent: extent,
            swapchain_image_views,
            render_pass,
            framebuffers,
            pipeline_layout,
            pipeline,
            descriptor_pool,
            descriptor_set_layout,
            ground_descriptor_set,
            platform_descriptor_set,
            player_descriptor_set,
            goal_descriptor_set,
            boss_descriptor_set,
            boss_hp_descriptor_set,
            fireball_descriptor_set,
            turtle_descriptor_set,
            hud_descriptor_set,
            vertex_buffer,
            vertex_buffer_memory,
            command_pool,
            command_buffer,
            image_available,
            render_finished,
            in_flight,
            _ground_texture_image: ground_image,
            _platform_texture_image: platform_image,
            _player_texture_image: player_image,
            _goal_texture_image: goal_image,
            _boss_texture_image: boss_image,
            _boss_hp_texture_image: boss_hp_image,
            _fireball_texture_image: fireball_image,
            _turtle_texture_image: turtle_image,
            _hud_texture_image: hud_image,
            sampler,
        })
    }

    unsafe fn draw_frame(&mut self, state: &GameState) {
        self.device.wait_for_fences(&[self.in_flight], true, u64::MAX).expect("wait_for_fences");

        let (image_index, _) = match self.swapchain_loader.acquire_next_image(self.swapchain, u64::MAX, self.image_available, vk::Fence::null()) {
            Ok(r) => r,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return,
            Err(e) => panic!("vkAcquireNextImageKHR failed: {e}"),
        };
        self.device.reset_fences(&[self.in_flight]).expect("reset_fences");

        let mut sprites_ndc = [[0.0f32; 4]; DRAWN_SPRITE_COUNT];
        sprites_ndc[0] = state.ground_dest_ndc();
        sprites_ndc[1] = state.platform_dest_ndc();
        sprites_ndc[2] = state.player_dest_ndc();
        sprites_ndc[3] = state.goal_dest_ndc();
        sprites_ndc[4] = state.boss_dest_ndc();
        sprites_ndc[5] = state.boss_hp_bar_ndc();
        for i in 0..MAX_FIREBALLS {
            sprites_ndc[FIREBALL_SPRITE_OFFSET + i] = state.fireball_dest_ndc(i);
        }
        for i in 0..TURTLE_COUNT {
            sprites_ndc[TURTLE_SPRITE_OFFSET + i] = state.turtle_dest_ndc(i);
        }
        sprites_ndc[HUD_SPRITE_INDEX] = [-1.0, 1.0 - HUD_HEIGHT_NDC, 1.0, 1.0];
        let mut all_verts = [Vertex { pos: [0.0; 3], uv: [0.0; 2] }; DRAWN_SPRITE_COUNT * VERTS_PER_SPRITE];
        for (i, ndc) in sprites_ndc.iter().enumerate() {
            let verts = quad_vertices(*ndc);
            all_verts[i * VERTS_PER_SPRITE..(i + 1) * VERTS_PER_SPRITE].copy_from_slice(&verts);
        }
        let vbuf_size = std::mem::size_of_val(&all_verts) as vk::DeviceSize;
        let ptr = self.device.map_memory(self.vertex_buffer_memory, 0, vbuf_size, vk::MemoryMapFlags::empty()).expect("map vertex buffer");
        std::ptr::copy_nonoverlapping(all_verts.as_ptr() as *const u8, ptr as *mut u8, vbuf_size as usize);
        self.device.unmap_memory(self.vertex_buffer_memory);

        self.device.reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty()).expect("reset command buffer");
        let begin_info = vk::CommandBufferBeginInfo::builder();
        self.device.begin_command_buffer(self.command_buffer, &begin_info).expect("begin command buffer");

        let clear_value = vk::ClearValue { color: vk::ClearColorValue { float32: [0.3, 0.6, 0.9, 1.0] } };
        let clear_values = [clear_value];
        let render_pass_begin = vk::RenderPassBeginInfo::builder()
            .render_pass(self.render_pass)
            .framebuffer(self.framebuffers[image_index as usize])
            .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: self.swapchain_extent })
            .clear_values(&clear_values);
        self.device.cmd_begin_render_pass(self.command_buffer, &render_pass_begin, vk::SubpassContents::INLINE);
        self.device.cmd_bind_pipeline(self.command_buffer, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
        self.device.cmd_bind_vertex_buffers(self.command_buffer, 0, &[self.vertex_buffer], &[0]);

        let mut sets = [self.ground_descriptor_set; DRAWN_SPRITE_COUNT];
        sets[0] = self.ground_descriptor_set;
        sets[1] = self.platform_descriptor_set;
        sets[2] = self.player_descriptor_set;
        sets[3] = self.goal_descriptor_set;
        sets[4] = self.boss_descriptor_set;
        sets[5] = self.boss_hp_descriptor_set;
        for i in 0..MAX_FIREBALLS {
            sets[FIREBALL_SPRITE_OFFSET + i] = self.fireball_descriptor_set;
        }
        for i in 0..TURTLE_COUNT {
            sets[TURTLE_SPRITE_OFFSET + i] = self.turtle_descriptor_set;
        }
        sets[HUD_SPRITE_INDEX] = self.hud_descriptor_set;
        for (i, &set) in sets.iter().enumerate() {
            self.device.cmd_bind_descriptor_sets(self.command_buffer, vk::PipelineBindPoint::GRAPHICS, self.pipeline_layout, 0, &[set], &[]);
            self.device.cmd_draw(self.command_buffer, VERTS_PER_SPRITE as u32, 1, (i * VERTS_PER_SPRITE) as u32, 0);
        }

        self.device.cmd_end_render_pass(self.command_buffer);
        self.device.end_command_buffer(self.command_buffer).expect("end command buffer");

        let wait_semaphores = [self.image_available];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let signal_semaphores = [self.render_finished];
        let command_buffers = [self.command_buffer];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores)
            .build();
        self.device.queue_submit(self.queue, &[submit_info], self.in_flight).expect("queue_submit");

        let swapchains = [self.swapchain];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHR::builder().wait_semaphores(&signal_semaphores).swapchains(&swapchains).image_indices(&image_indices);
        match self.swapchain_loader.queue_present(self.queue, &present_info) {
            Ok(_) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {}
            Err(e) => panic!("vkQueuePresentKHR failed: {e}"),
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_semaphore(self.image_available, None);
            self.device.destroy_semaphore(self.render_finished, None);
            self.device.destroy_fence(self.in_flight, None);
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_descriptor_pool(self.descriptor_pool, None);
            self.device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            self.device.destroy_sampler(self.sampler, None);
            self.device.destroy_buffer(self.vertex_buffer, None);
            self.device.free_memory(self.vertex_buffer_memory, None);
            for &fb in &self.framebuffers {
                self.device.destroy_framebuffer(fb, None);
            }
            self.device.destroy_render_pass(self.render_pass, None);
            for &view in &self.swapchain_image_views {
                self.device.destroy_image_view(view, None);
            }
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

unsafe fn create_host_visible_buffer(
    device: &ash::Device,
    memory_properties: &vk::PhysicalDeviceMemoryProperties,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
) -> Result<(vk::Buffer, vk::DeviceMemory), String> {
    let buffer_info = vk::BufferCreateInfo::builder().size(size).usage(usage).sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer = device.create_buffer(&buffer_info, None).map_err(|e| e.to_string())?;
    let mem_req = device.get_buffer_memory_requirements(buffer);
    let mem_type = find_memory_type(memory_properties, mem_req.memory_type_bits, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
        .ok_or("no suitable host-visible memory type")?;
    let alloc_info = vk::MemoryAllocateInfo::builder().allocation_size(mem_req.size).memory_type_index(mem_type);
    let memory = device.allocate_memory(&alloc_info, None).map_err(|e| e.to_string())?;
    device.bind_buffer_memory(buffer, memory, 0).map_err(|e| e.to_string())?;
    Ok((buffer, memory))
}

fn find_memory_type(props: &vk::PhysicalDeviceMemoryProperties, type_bits: u32, required: vk::MemoryPropertyFlags) -> Option<u32> {
    (0..props.memory_type_count).find(|&i| type_bits & (1 << i) != 0 && props.memory_types[i as usize].property_flags.contains(required))
}

unsafe fn create_solid_texture(
    device: &ash::Device,
    memory_properties: &vk::PhysicalDeviceMemoryProperties,
    queue: vk::Queue,
    queue_family_index: u32,
    color: Rgba8,
) -> Result<(vk::Image, vk::ImageView), String> {
    create_texture_from_rgba(device, memory_properties, queue, queue_family_index, &TextureRgba8 { width: 1, height: 1, pixels: vec![color] })
}

/// 任意サイズのRGBA8テクスチャを実際にアップロードする(`create_solid_texture`
/// の1x1専用実装を一般化したもの、HUDバナー画像等の任意画像に使う)。
unsafe fn create_texture_from_rgba(
    device: &ash::Device,
    memory_properties: &vk::PhysicalDeviceMemoryProperties,
    queue: vk::Queue,
    queue_family_index: u32,
    texture: &TextureRgba8,
) -> Result<(vk::Image, vk::ImageView), String> {
    const FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
    let (width, height) = (texture.width, texture.height);

    let tex_bytes_size = (width as u64) * (height as u64) * 4;
    let (staging_buffer, staging_memory) = create_host_visible_buffer(device, memory_properties, tex_bytes_size, vk::BufferUsageFlags::TRANSFER_SRC)?;
    let ptr = device.map_memory(staging_memory, 0, tex_bytes_size, vk::MemoryMapFlags::empty()).map_err(|e| e.to_string())?;
    let dst = std::slice::from_raw_parts_mut(ptr as *mut u8, tex_bytes_size as usize);
    for (i, p) in texture.pixels.iter().enumerate() {
        dst[i * 4..i * 4 + 4].copy_from_slice(&[p.r, p.g, p.b, p.a]);
    }
    device.unmap_memory(staging_memory);

    let image_info = vk::ImageCreateInfo::builder()
        .image_type(vk::ImageType::TYPE_2D)
        .format(FORMAT)
        .extent(vk::Extent3D { width, height, depth: 1 })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    let image = device.create_image(&image_info, None).map_err(|e| e.to_string())?;
    let mem_req = device.get_image_memory_requirements(image);
    let mem_type = find_memory_type(memory_properties, mem_req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL).ok_or("no device-local memory type")?;
    let alloc_info = vk::MemoryAllocateInfo::builder().allocation_size(mem_req.size).memory_type_index(mem_type);
    let memory = device.allocate_memory(&alloc_info, None).map_err(|e| e.to_string())?;
    device.bind_image_memory(image, memory, 0).map_err(|e| e.to_string())?;

    let view_info = vk::ImageViewCreateInfo::builder()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(FORMAT)
        .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 });
    let view = device.create_image_view(&view_info, None).map_err(|e| e.to_string())?;

    let pool_info = vk::CommandPoolCreateInfo::builder().queue_family_index(queue_family_index);
    let pool = device.create_command_pool(&pool_info, None).map_err(|e| e.to_string())?;
    let cmd_alloc_info = vk::CommandBufferAllocateInfo::builder().command_pool(pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1);
    let cmd = device.allocate_command_buffers(&cmd_alloc_info).map_err(|e| e.to_string())?[0];
    let begin_info = vk::CommandBufferBeginInfo::builder();
    device.begin_command_buffer(cmd, &begin_info).map_err(|e| e.to_string())?;

    let subresource = vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 };
    let to_dst = vk::ImageMemoryBarrier::builder()
        .old_layout(vk::ImageLayout::UNDEFINED)
        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
        .image(image)
        .subresource_range(subresource)
        .build();
    device.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::TRANSFER, vk::DependencyFlags::empty(), &[], &[], &[to_dst]);
    let copy_region = vk::BufferImageCopy {
        buffer_offset: 0,
        buffer_row_length: 0,
        buffer_image_height: 0,
        image_subresource: vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 },
        image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
        image_extent: vk::Extent3D { width, height, depth: 1 },
    };
    device.cmd_copy_buffer_to_image(cmd, staging_buffer, image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[copy_region]);
    let to_read = vk::ImageMemoryBarrier::builder()
        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
        .dst_access_mask(vk::AccessFlags::SHADER_READ)
        .image(image)
        .subresource_range(subresource)
        .build();
    device.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(), &[], &[], &[to_read]);
    device.end_command_buffer(cmd).map_err(|e| e.to_string())?;

    let fence = device.create_fence(&vk::FenceCreateInfo::builder(), None).map_err(|e| e.to_string())?;
    let cmds = [cmd];
    let submit = vk::SubmitInfo::builder().command_buffers(&cmds).build();
    device.queue_submit(queue, &[submit], fence).map_err(|e| e.to_string())?;
    device.wait_for_fences(&[fence], true, u64::MAX).map_err(|e| e.to_string())?;

    device.destroy_fence(fence, None);
    device.destroy_command_pool(pool, None);
    device.destroy_buffer(staging_buffer, None);
    device.free_memory(staging_memory, None);

    Ok((image, view))
}
