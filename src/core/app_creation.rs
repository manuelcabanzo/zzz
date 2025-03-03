use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use crate::core::file_system::FileSystem;
use crate::core::android_resources::AndroidResources;
use crate::core::android_sdk_manager::AndroidSdkManager;

pub struct AppCreation {
    pub app_name: String,
    pub app_path: String,
    pub api_level: String,
    resources: AndroidResources,
    logger: Arc<dyn Fn(String) + Send + Sync>,
    progress_callback: Arc<dyn Fn(f32) + Send + Sync>,
}

impl AppCreation {
    pub fn new(
        app_name: String, 
        app_path: String, 
        api_level: String, 
        logger: Arc<dyn Fn(String) + Send + Sync>, 
        progress_callback: Arc<dyn Fn(f32) + Send + Sync>
    ) -> Self {
        let resources = AndroidResources::load_state()
            .unwrap_or_else(|_| AndroidResources::new());
        
        Self {
            app_name,
            app_path,
            api_level,
            resources,
            logger,
            progress_callback,
        }
    }

    pub fn create_app(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.app_name.is_empty() || self.app_path.is_empty() {
            return Ok(());
        }

        (self.logger)("Starting app creation...".to_string());

        // Initialize SDK manager and accept licenses
        let sdk_manager = AndroidSdkManager::new();
        let progress_callback = Arc::clone(&self.progress_callback);

        // Create runtime for async operations
        let rt = Runtime::new()?;
        rt.block_on(async {
            // Ensure SDK is downloaded and licenses accepted
            (self.logger)("Setting up Android SDK...".to_string());
            sdk_manager.accept_licenses()?;
            (self.logger)("Downloading Android SDK...".to_string());
            sdk_manager.ensure_api_level(&self.api_level, progress_callback).await?;
            (self.logger)("SDK setup completed".to_string());
            Ok::<_, Box<dyn std::error::Error>>(())
        })?;

        // Create root project directory
        (self.logger)("Creating project structure...".to_string());
        let project_dir = PathBuf::from(&self.app_path).join(&self.app_name);
        let app_dir = project_dir.join("app"); // Create app subdirectory
        let fs = Arc::new(FileSystem::new(project_dir.to_str().unwrap()));
        
        // Create root and app directories
        fs.create_directory(&project_dir)?;
        fs.create_directory(&app_dir)?;
        (self.logger)("Created root directories".to_string());
        (self.progress_callback)(0.5);

        // Move all Android-specific directories under app/
        (self.logger)("Creating Android app directories...".to_string());
        let src_main_dir = app_dir.join("src").join("main");
        let src_test_dir = app_dir.join("src").join("test");
        let src_android_test_dir = app_dir.join("src").join("androidTest");
        let kotlin_dir = src_main_dir.join("kotlin").join("com").join("example").join("app");
        let res_dir = src_main_dir.join("res");
        let java_test_dir = src_test_dir.join("java").join("com").join("example").join("app");
        let java_android_test_dir = src_android_test_dir.join("java").join("com").join("example").join("app");
    
        // Create all necessary directories
        for (i, dir) in [
            &src_main_dir,
            &src_test_dir,
            &src_android_test_dir,
            &kotlin_dir,
            &res_dir,
            &java_test_dir,
            &java_android_test_dir,
            &res_dir.join("layout"),
            &res_dir.join("values"),
            &res_dir.join("values-night"),
            &res_dir.join("xml"),
            &res_dir.join("mipmap-mdpi"),
            &res_dir.join("mipmap-hdpi"),
            &res_dir.join("mipmap-xhdpi"),
            &res_dir.join("mipmap-xxhdpi"),
            &res_dir.join("mipmap-xxxhdpi"),
        ].iter().enumerate() {
            fs.create_directory(dir)?;
            (self.progress_callback)(0.5 + 0.05 * i as f32);
        }

        // Create basic launcher icons
        let icon_sizes = [
            ("mipmap-mdpi", 48),
            ("mipmap-hdpi", 72),
            ("mipmap-xhdpi", 96),
            ("mipmap-xxhdpi", 144),
            ("mipmap-xxxhdpi", 192),
        ];

        for (dir_name, size) in icon_sizes {
            // Create default launcher icon (square background)
            let icon_content = format!(
                r#"<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@android:color/white"/>
    <foreground>
        <inset
            android:drawable="@android:color/holo_blue_dark"
            android:inset="{}"/>
    </foreground>
</adaptive-icon>"#,
                size / 4
            );

            // Save both regular and round icons
            fs::write(
                res_dir.join(dir_name).join("ic_launcher.xml"),
                &icon_content
            )?;
            fs::write(
                res_dir.join(dir_name).join("ic_launcher_round.xml"),
                &icon_content
            )?;
        }

        // Create the base icon drawable
        let drawable_dir = res_dir.join("drawable");
        fs::create_dir_all(&drawable_dir)?;
        let base_icon = r#"<?xml version="1.0" encoding="utf-8"?>
<shape xmlns:android="http://schemas.android.com/apk/res/android"
    android:shape="rectangle">
    <solid android:color="@android:color/holo_blue_dark"/>
    <corners android:radius="8dp"/>
</shape>"#;
        fs::write(drawable_dir.join("ic_launcher_foreground.xml"), base_icon)?;

        // Ensure Gradle files exist before copying
        (self.logger)("Setting up Gradle build system...".to_string());
        self.resources.ensure_gradle_files()?;
        
        let gradle_source = self.resources.get_gradle_path();
        let gradle_wrapper_dir = project_dir.join("gradle").join("wrapper");
        fs.create_directory(&gradle_wrapper_dir)?;

        // Copy all Gradle files
        let gradle_files = [
            (gradle_source.join("gradlew"), project_dir.join("gradlew")),
            (gradle_source.join("gradlew.bat"), project_dir.join("gradlew.bat")),
            (gradle_source.join("wrapper").join("gradle-wrapper.jar"), 
             gradle_wrapper_dir.join("gradle-wrapper.jar")),
            (gradle_source.join("wrapper").join("gradle-wrapper.properties"), 
             gradle_wrapper_dir.join("gradle-wrapper.properties")),
        ];

        for (source, dest) in gradle_files.iter() {
            if !source.exists() {
                return Err(format!("Gradle file not found: {}", source.display()).into());
            }
            fs::copy(source, dest)?;
            
            #[cfg(unix)]
            if dest.file_name().map_or(false, |f| f == "gradlew") {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(dest)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(dest, perms)?;
            }
        }
        (self.progress_callback)(0.6);
    
        // Create root build.gradle.kts
        (self.logger)("Creating build configuration files...".to_string());
        let root_build_gradle = r#"buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath("com.android.tools.build:gradle:8.2.1")
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:1.9.0")
    }
}"#;
        fs::write(project_dir.join("build.gradle.kts"), root_build_gradle)?;

        // Move build.gradle.kts content to app/build.gradle.kts
        let app_build_gradle = format!(
            r#"plugins {{
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}}

android {{
    namespace = "com.example.app"
    compileSdk = {api_level}
    // ...rest of existing android config...
}}
"#,
            api_level = self.api_level
        );
        fs::write(app_dir.join("build.gradle.kts"), app_build_gradle)?;

        // Update settings.gradle.kts
        let settings_gradle = format!(r#"pluginManagement {{
    repositories {{
        google()
        mavenCentral()
        gradlePluginPortal()
    }}
}}
dependencyResolutionManagement {{
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {{
        google()
        mavenCentral()
    }}
}}

rootProject.name = "{}"
include(":app")
"#, self.app_name);
        fs::write(project_dir.join("settings.gradle.kts"), settings_gradle)?;

        // Create local.properties with SDK path
        let sdk_path = sdk_manager.get_sdk_path();
        let local_properties = format!("sdk.dir={}", sdk_path.to_str().unwrap().replace("\\", "\\\\"));
        fs::write(project_dir.join("local.properties"), local_properties)?;

        // Create build.gradle.kts
        let build_gradle_content = format!(
            r#"plugins {{
        id("com.android.application")
        id("org.jetbrains.kotlin.android")
    }}
    
    android {{
        namespace = "com.example.app"
        compileSdk = {api_level}
    
        defaultConfig {{
            applicationId = "com.example.app"
            minSdk = 24
            targetSdk = {api_level}
            versionCode = 1
            versionName = "1.0"
    
            testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
            vectorDrawables {{
                useSupportLibrary = true
            }}
        }}
    
        buildTypes {{
            release {{
                isMinifyEnabled = false
                proguardFiles(
                    getDefaultProguardFile("proguard-android-optimize.txt"),
                    "proguard-rules.pro"
                )
            }}
        }}
        
        compileOptions {{
            sourceCompatibility = JavaVersion.VERSION_17
            targetCompatibility = JavaVersion.VERSION_17
        }}
        
        kotlinOptions {{
            jvmTarget = "17"
        }}
        
        buildFeatures {{
            compose = true
        }}
        
        composeOptions {{
            kotlinCompilerExtensionVersion = "1.5.1"
        }}
        
        packaging {{
            resources {{
                excludes += "/META-INF/{{AL2.0,LGPL2.1}}"
            }}
        }}
    }}
    
    dependencies {{
        implementation("androidx.core:core-ktx:1.12.0")
        implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.7.0")
        implementation("androidx.activity:activity-compose:1.8.2")
        implementation(platform("androidx.compose:compose-bom:2023.08.00"))
        implementation("androidx.compose.ui:ui")
        implementation("androidx.compose.ui:ui-graphics")
        implementation("androidx.compose.ui:ui-tooling-preview")
        implementation("androidx.compose.material3:material3")
        testImplementation("junit:junit:4.13.2")
        androidTestImplementation("androidx.test.ext:junit:1.1.5")
        androidTestImplementation("androidx.test.espresso:espresso-core:3.5.1")
        androidTestImplementation(platform("androidx.compose:compose-bom:2023.08.00"))
        androidTestImplementation("androidx.compose.ui:ui-test-junit4")
        debugImplementation("androidx.compose.ui:ui-tooling")
        debugImplementation("androidx.compose.ui:ui-test-manifest")
    }}
    "#,
            api_level = self.api_level
        );
        fs::write(app_dir.join("build.gradle.kts"), build_gradle_content)?;
        (self.progress_callback)(0.8);
    
        // Create settings.gradle.kts
        let settings_gradle_content = r#"pluginManagement {
        repositories {
            google()
            mavenCentral()
            gradlePluginPortal()
        }
    }
    dependencyResolutionManagement {
        repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
        repositories {
            google()
            mavenCentral()
        }
    }
    
    rootProject.name = "MyApplication"
    include(":app")
    "#;
        fs::write(project_dir.join("settings.gradle.kts"), settings_gradle_content)?;
    
        // Create gradle.properties with AndroidX configuration
        (self.logger)("Creating Gradle configuration...".to_string());
        let gradle_properties = r#"org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8
android.useAndroidX=true
android.enableJetifier=true
kotlin.code.style=official
org.gradle.parallel=true
org.gradle.caching=true
"#;
        fs::write(project_dir.join("gradle.properties"), gradle_properties)?;

        // Create MainActivity.kt
        (self.logger)("Creating Android source files...".to_string());
        let main_activity_content = r#"package com.example.app
    
    import android.os.Bundle
    import androidx.activity.ComponentActivity
    import androidx.activity.compose.setContent
    import androidx.compose.foundation.layout.fillMaxSize
    import androidx.compose.material3.MaterialTheme
    import androidx.compose.material3.Surface
    import androidx.compose.material3.Text
    import androidx.compose.runtime.Composable
    import androidx.compose.ui.Modifier
    import androidx.compose.ui.tooling.preview.Preview
    
    class MainActivity : ComponentActivity() {
        override fun onCreate(savedInstanceState: Bundle?) {
            super.onCreate(savedInstanceState)
            setContent {
                AppTheme {
                    Surface(
                        modifier = Modifier.fillMaxSize(),
                        color = MaterialTheme.colorScheme.background
                    ) {
                        Greeting("Android")
                    }
                }
            }
        }
    }
    
    @Composable
    fun Greeting(name: String, modifier: Modifier = Modifier) {
        Text(
            text = "Hello $name!",
            modifier = modifier
        )
    }
    
    @Preview(showBackground = true)
    @Composable
    fun GreetingPreview() {
        AppTheme {
            Greeting("Android")
        }
    }
    "#;
        fs::write(kotlin_dir.join("MainActivity.kt"), main_activity_content)?;
    
        // Create Theme.kt
        let theme_content = r#"package com.example.app
    
    import android.app.Activity
    import android.os.Build
    import androidx.compose.foundation.isSystemInDarkTheme
    import androidx.compose.material3.MaterialTheme
    import androidx.compose.material3.darkColorScheme
    import androidx.compose.material3.dynamicDarkColorScheme
    import androidx.compose.material3.dynamicLightColorScheme
    import androidx.compose.material3.lightColorScheme
    import androidx.compose.runtime.Composable
    import androidx.compose.runtime.SideEffect
    import androidx.compose.ui.graphics.toArgb
    import androidx.compose.ui.platform.LocalContext
    import androidx.compose.ui.platform.LocalView
    import androidx.core.view.WindowCompat
    
    private val DarkColorScheme = darkColorScheme()
    private val LightColorScheme = lightColorScheme()
    
    @Composable
    fun AppTheme(
        darkTheme: Boolean = isSystemInDarkTheme(),
        dynamicColor: Boolean = true,
        content: @Composable () -> Unit
    ) {
        val colorScheme = when {
            dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
                val context = LocalContext.current
                if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
            }
            darkTheme -> DarkColorScheme
            else -> LightColorScheme
        }
        val view = LocalView.current
        if (!view.isInEditMode) {
            SideEffect {
                val window = (view.context as Activity).window
                window.statusBarColor = colorScheme.primary.toArgb()
                WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = darkTheme
            }
        }
    
        MaterialTheme(
            colorScheme = colorScheme,
            content = content
        )
    }
    "#;
        fs::write(kotlin_dir.join("Theme.kt"), theme_content)?;
    
        // Create AndroidManifest.xml
        (self.logger)("Creating Android resource files...".to_string());
        let manifest_content = r#"<?xml version="1.0" encoding="utf-8"?>
    <manifest xmlns:android="http://schemas.android.com/apk/res/android"
        xmlns:tools="http://schemas.android.com/tools">
    
        <application
            android:allowBackup="true"
            android:dataExtractionRules="@xml/data_extraction_rules"
            android:fullBackupContent="@xml/backup_rules"
            android:icon="@mipmap/ic_launcher"
            android:label="@string/app_name"
            android:roundIcon="@mipmap/ic_launcher_round"
            android:supportsRtl="true"
            android:theme="@style/Theme.App"
            tools:targetApi="31">
            <activity
                android:name=".MainActivity"
                android:exported="true"
                android:theme="@style/Theme.App">
                <intent-filter>
                    <action android:name="android.intent.action.MAIN" />
                    <category android:name="android.intent.category.LAUNCHER" />
                </intent-filter>
            </activity>
        </application>
    </manifest>"#;
        fs::write(src_main_dir.join("AndroidManifest.xml"), manifest_content)?;
    
        // Create strings.xml
        let strings_content = format!(r#"<?xml version="1.0" encoding="utf-8"?>
    <resources>
        <string name="app_name">{}</string>
    </resources>"#, self.app_name);
        fs::write(res_dir.join("values").join("strings.xml"), strings_content)?;
    
        // Create themes.xml
        let themes_content = r#"<?xml version="1.0" encoding="utf-8"?>
    <resources>
        <style name="Theme.App" parent="android:Theme.Material.Light.NoActionBar" />
    </resources>"#;
        fs::write(res_dir.join("values").join("themes.xml"), themes_content)?;
    
        // Create backup_rules.xml
        let backup_rules_content = r#"<?xml version="1.0" encoding="utf-8"?>
    <full-backup-content>
        <include domain="sharedpref" path="."/>
        <exclude domain="sharedpref" path="device.xml"/>
    </full-backup-content>"#;
        fs::write(res_dir.join("xml").join("backup_rules.xml"), backup_rules_content)?;
    
        // Create data_extraction_rules.xml
        let data_extraction_content = r#"<?xml version="1.0" encoding="utf-8"?>
    <data-extraction-rules>
        <cloud-backup>
            <include domain="sharedpref" path="."/>
            <exclude domain="sharedpref" path="device.xml"/>
        </cloud-backup>
        <device-transfer>
            <include domain="sharedpref" path="."/>
            <exclude domain="sharedpref" path="device.xml"/>
        </device-transfer>
    </data-extraction-rules>"#;
        fs::write(res_dir.join("xml").join("data_extraction_rules.xml"), data_extraction_content)?;
    
        // Create ExampleUnitTest.kt
        (self.logger)("Creating test files...".to_string());
        let unit_test_content = r#"package com.example.app
    
    import org.junit.Test
    import org.junit.Assert.*
    
    class ExampleUnitTest {
        @Test
        fun addition_isCorrect() {
            assertEquals(4, 2 + 2)
        }
    }"#;
        fs::write(java_test_dir.join("ExampleUnitTest.kt"), unit_test_content)?;
    
        // Create ExampleInstrumentedTest.kt
        let instrumented_test_content = r#"package com.example.app
    
    import androidx.test.platform.app.InstrumentationRegistry
    import androidx.test.ext.junit.runners.AndroidJUnit4
    import org.junit.Test
    import org.junit.runner.RunWith
    import org.junit.Assert.*
    
    @RunWith(AndroidJUnit4::class)
    class ExampleInstrumentedTest {
        @Test
        fun useAppContext() {
            val appContext = InstrumentationRegistry.getInstrumentation().targetContext
            assertEquals("com.example.app", appContext.packageName)
        }
    }"#;
        fs::write(java_android_test_dir.join("ExampleInstrumentedTest.kt"), instrumented_test_content)?;
    
        // Create .gitignore
        let gitignore_content = r#"*.iml
    .gradle
    /local.properties
    /.idea
    .DS_Store
    /build
    /captures
    .externalNativeBuild
    .cxx
    local.properties"#;
        fs::write(project_dir.join(".gitignore"), gitignore_content)?;
    
        // Save resources state
        (self.logger)("Finalizing project setup...".to_string());
        self.resources.save_state()?;
        (self.progress_callback)(1.0);
        (self.logger)(format!("App creation completed. Project created at: {}", project_dir.display()));
    
        Ok(())
    }

}

